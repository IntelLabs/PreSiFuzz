// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(test, deny(dead_code, unreachable_code))]
#![allow(dead_code, unreachable_code)]

use std::path::PathBuf;

#[cfg(feature = "tui")]
use libafl::monitors::tui::TuiMonitor;

// #[cfg(not(feature = "tui"))]
use libafl::executors::command::CommandConfigurator;
use libafl::state::HasMaxSize;
use libafl::{
    corpus::{InMemoryCorpus, OnDiskCorpus},
    events::EventConfig,
    events::SimpleEventManager,
    feedback_and_fast, feedback_not, feedback_or,
    feedbacks::MaxMapFeedback,
    fuzzer::{Fuzzer, StdFuzzer},
    inputs::BytesInput,
    monitors::multi::MultiMonitor,
    monitors::SimpleMonitor,
    observers::{HitcountsMapObserver, StdMapObserver},
    schedulers::QueueScheduler,
    stages::StdMutationalStage,
    state::StdState,
    Error, HasFeedback,
};

use tempdir::TempDir;

#[cfg(not(target_vendor = "apple"))]
use libafl_bolts::shmem::StdShMemProvider;
#[cfg(target_vendor = "apple")]
use libafl_bolts::shmem::UnixShMemProvider;
use libafl_bolts::{
    core_affinity::Cores,
    current_nanos,
    rands::StdRand,
    shmem::{ShMem, ShMemProvider},
    tuples::tuple_list,
    AsMutSlice,
};
#[cfg(feature = "std")]
use std::net::{SocketAddr, ToSocketAddrs};

#[cfg(feature = "debug")]
use color_print::cprintln;

use std::os::unix::fs;

use std::env;
use std::io::Result;
use std::path::Path;
use std::rc::Rc;

use clap::AppSettings;
use clap::{App, Arg};

//use libpresifuzz_feedbacks::verdi_feedback::VerdiFeedback;

use libpresifuzz_feedbacks::verdi_xml_feedback::VerdiFeedback;
use libpresifuzz_observers::verdi_xml_observer::VerdiCoverageMetric;
use libpresifuzz_observers::verdi_xml_observer::VerdiCoverageMetric::*;
use libpresifuzz_observers::verdi_xml_observer::VerdiXMLMapObserver;

pub mod simv;
use crate::simv::SimvCommandConfigurator;

use libpresifuzz_mutators::riscv_isa::riscv_mutations;
use libpresifuzz_mutators::scheduled::StdISAScheduledMutator;

use libpresifuzz_ec::llmp::Launcher;
use libpresifuzz_ec::manager::*;
use libpresifuzz_feedbacks::transferred::TransferredFeedback;
use libpresifuzz_stages::sync::SyncFromDiskStage;

mod differential;
mod differential_feedback;
//mod verdi_feedback;
//use crate::verdi_feedback::VerdiFeedback;

#[derive(Debug)]
pub struct WorkDir(Option<TempDir>);

// Forward inherent methods to the tempdir crate.
impl WorkDir {
    pub fn new(prefix: &str) -> Result<WorkDir> {
        TempDir::new(prefix).map(Some).map(WorkDir)
    }

    pub fn path(&self) -> &Path {
        self.0.as_ref().unwrap().path()
    }
}

/// Leaks the inner TempDir if we are unwinding.
impl Drop for WorkDir {
    fn drop(&mut self) {
        ::std::mem::forget(self.0.take())
    }
}

pub fn symlink_files(from: Vec<&str>, to: Vec<&str>, workdir: &str) {
    let current_dir = env::current_dir()
        .unwrap()
        .as_os_str()
        .to_str()
        .unwrap()
        .to_string();

    for i in 0..from.len() {
        #[cfg(feature = "debug")]
        cprintln!(
            "<green>[INFO]</green> symbolic link for {}/{} to {}/{}",
            current_dir,
            from[i],
            workdir,
            to[i]
        );

        let src = format!("{}/{}", current_dir, from[i]);
        let dst = format!("{}/{}", workdir, to[i]);

        fs::symlink(&src, &dst).expect("Fail to create symlink for yaml file to workdir!");
    }
}

/// The actual fuzzer
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
#[allow(clippy::similar_names)]
pub fn main() {
    // color_backtrace::install();

    fuzz();
}

macro_rules! create_verdi_observer_and_feedback {
    ($fdb:ident, $obs:ident, $metric:ident, $minimized:expr, $workdir:ident) => {
        let ($obs, $fdb) = {
            let verdi_observer = VerdiXMLMapObserver::new(
                concat!("verdi_", stringify!($metric)),
                &"Coverage.vdb".to_string(),
                &$workdir.to_string(),
                match $metric {
                    Toggle => VerdiCoverageMetric::Toggle,
                    Branch => VerdiCoverageMetric::Branch,
                    Line => VerdiCoverageMetric::Line,
                    Condition => VerdiCoverageMetric::Condition,
                    FSM => VerdiCoverageMetric::FSM,
                    _ => VerdiCoverageMetric::Toggle,
                },
                &"".to_string(), //&"ariane_tb.dut.i_ariane".to_string()
            );

            let feedback = VerdiFeedback::new_with_observer(
                concat!("verdi_", stringify!($metric)),
                &$workdir.to_string(),
                $minimized,
            );
            (feedback, verdi_observer)
        };
    };
}

pub fn fuzz() {
    let yaml_fd = std::fs::File::open("config.yml").unwrap();
    let config: serde_yaml::Value = serde_yaml::from_reader(yaml_fd).unwrap();

    let max_testcase_size: usize = config["fuzzer"]["max_testcase_size"]
        .as_u64()
        .unwrap_or(1024)
        .try_into()
        .unwrap();

    // allocate the shared memory provider for later use
    #[cfg(target_vendoe = "apple")]
    let mut shmem_provider = UnixShMemProvider::new().unwrap();
    #[cfg(not(target_vendor = "apple"))]
    let shmem_provider = StdShMemProvider::new().unwrap();
    let mut shmem_provider_client = shmem_provider.clone();

    let mon = MultiMonitor::new(|s| println!("{s}"));

    let sync_dir = format!("{}/sync/", std::env::current_dir().unwrap().display());
    println!("sync_dir: {}", sync_dir);

    let corpus_dir = format!("{}/seeds", env::current_dir().unwrap().display());

    let mut run_client = |_state: Option<_>, mut mgr, _core_id| {
        // get a unique temp-dir name
        let tmp_dir = WorkDir::new("presifuzz_").expect("Unable to create temporary directory");
        let workdir = tmp_dir.path().as_os_str().to_str().unwrap().to_owned();

        let workdir: &str = workdir.as_str();

        symlink_files(
            vec!["config.yml", "run.sh"],
            vec!["config.yml", "run.sh"],
            workdir,
        );

        let simv =
            SimvCommandConfigurator::new_from_config_file("config.yml", workdir, &mut [], "", 1);

        std::env::set_current_dir(&workdir).expect("Unable to change into {dir}");
        let mut spike_trace_observer = ExecTraceObserver::<ProcessorfuzzExecTraceObserver>::new("spike_exec_trace_observer", "./spike.log");
        let mut rocket_trace_observer = ExecTraceObserver::<CVA6ExecTraceObserver>::new("cva6_exec_trace_observer", "./cva6.log");

        let mut objective = differential_feedback::DifferentialFeedback::new_with_observer(
            "spike_exec_trace_observer",
            "cva6_exec_trace_observer",
            "differential_trace_feedback",
        );
        
        let processorfuzz_feedback = ProcessorFuzzFeedback::new("spike_exec_trace_observer");
        let mut feedback = feedback_or!(
            processorfuzz_feedback,
        );

        // Instantiate State with feedback, objective, in/out corpus
        let mut state = StdState::new(
            StdRand::with_seed(current_nanos()),
            OnDiskCorpus::<BytesInput>::new(&PathBuf::from("./corpus")).unwrap(),
            InMemoryCorpus::new(),
            &mut feedback,
            &mut objective,
        )
        .unwrap();
        state.set_max_size(max_testcase_size);

        // Simle FIFO scheduler
        let scheduler = QueueScheduler::new();

        // RISCV ISA mutator
        let mutator = StdISAScheduledMutator::new(riscv_mutations());

        // Finally, instantiate the fuzzer
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

        let mut executor = differential::DiffExecutor::new(
            spike.into_executor(tuple_list!()),
            simv.into_executor(tuple_list!()),
            tuple_list!(spike_trace_observer, rocket_trace_observer),
        );

        let corpus_dir = PathBuf::from(corpus_dir.to_string());

        // load initial inputs if any seeds provided
        state
            .load_initial_inputs(&mut fuzzer, &mut executor, &mut mgr, &[corpus_dir.clone()])
            .unwrap();

        // Instantiate a mutational stage that will apply mutations to the selected testcase
        let sync_dir = PathBuf::from(sync_dir.to_string());
        //let mut stages = tuple_list!(SyncFromDiskStage::new(sync_dir), StdMutationalStage::with_max_iterations(mutator, 1));
        let mut stages = tuple_list!(
            SyncFromDiskStage::new(sync_dir),
            StdMutationalStage::new(mutator)
        );

        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in fuzzing loop");

        Ok(())
    };

    match Launcher::<_, _, _, BytesInput>::builder()
        .configuration(EventConfig::from_name("default"))
        .monitor(mon)
        .run_client(&mut run_client)
        .stdout_file(Some("/dev/null"))
        .sync_dir(sync_dir.clone())
        .build()
        .launch()
    {
        Ok(()) => (),
        Err(Error::ShuttingDown) => (),
        Err(err) => panic!("Fuzzingg failed {err:?}"),
    };
}
