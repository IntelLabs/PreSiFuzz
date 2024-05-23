// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(
    test,
    deny(
        dead_code,
        unreachable_code
    )
)]
#![allow(dead_code, unreachable_code)]

use std::path::PathBuf;

#[cfg(feature = "tui")]
use libafl::monitors::tui::TuiMonitor;

// #[cfg(not(feature = "tui"))]
use libafl::{
    corpus::{OnDiskCorpus, InMemoryCorpus},
    events::{EventConfig},
    fuzzer::{Fuzzer, StdFuzzer},
    schedulers::QueueScheduler,
    events::{SimpleEventManager},
    monitors::SimpleMonitor,
    state::StdState,
    observers::{HitcountsMapObserver, StdMapObserver},
    feedbacks::MaxMapFeedback,
    inputs::BytesInput,
    monitors::multi::MultiMonitor,
    HasFeedback,
    feedback_not, feedback_and_fast, feedback_or,
    Error,
    stages::{
        StdMutationalStage
    },
};
use libafl::executors::command::CommandConfigurator;
use libafl::state::HasMaxSize;

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
        AsMutSlice
};
#[cfg(feature = "std")]
use std::{
    net::{SocketAddr, ToSocketAddrs}
};

#[cfg(feature = "debug")]
use color_print::cprintln;

use std::os::unix::fs;

use std::io::Result;
use std::path::Path;
use std::env;
use std::rc::Rc;

use clap::{App, Arg};
use clap::AppSettings;

use libpresifuzz_feedbacks::verdi_feedback::VerdiFeedback;
use libpresifuzz_feedbacks::sim_time_feedback::SimTimeFeedback;

use libpresifuzz_observers::verdi_observer::VerdiShMapObserver;
use libpresifuzz_observers::verdi_observer::VerdiCoverageMetric;

pub mod simv;
use crate::simv::SimvCommandConfigurator;

use libpresifuzz_observers::xtrace_observer::XTraceObserver;
use libpresifuzz_observers::stats_observer::StatsObserver;

use libpresifuzz_mutators::riscv_isa::riscv_mutations;
use libpresifuzz_mutators::scheduled::StdISAScheduledMutator;

use libpresifuzz_ec::manager::*;
use libpresifuzz_ec::llmp::Launcher;

use libpresifuzz_stages::mutational_objdump_sanitized::StdObjdumpSanitizedMutationalStage;
use libpresifuzz_stages::sync::SyncFromDiskStage;
use libpresifuzz_feedbacks::transferred::TransferredFeedback;

mod differential_feedback;
mod differential;

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
    
    let current_dir = env::current_dir().unwrap().as_os_str().to_str().unwrap().to_string(); 

    for i in 0..from.len(){

        #[cfg(feature = "debug")]
        cprintln!("<green>[INFO]</green> symbolic link for {}/{} to {}/{}", current_dir, from[i], workdir, to[i]);
    
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

pub fn fuzz() {

    let yaml_fd = std::fs::File::open("config.yml").unwrap();
    let config: serde_yaml::Value = serde_yaml::from_reader(yaml_fd).unwrap();
    
    let max_testcase_size: usize = config["fuzzer"]["max_testcase_size"]
        .as_u64()
        .unwrap_or(1024).try_into().unwrap();
    
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
        
        symlink_files(vec!["config.yml", "run.sh"], vec!["config.yml", "run.sh"], workdir);

        let simv = SimvCommandConfigurator::new_from_config_file("config.yml", workdir, &mut [], "", 1);
           
        std::env::set_current_dir(&workdir).expect("Unable to change into {dir}");

        // allocate a shared memory for coverage map
        const MAP_SIZE: usize = 1024 * 232;
        let mut shmem = shmem_provider_client.new_shmem(MAP_SIZE).unwrap();
        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
        
        // create verdi observer and feedback
        // monitor Toogle coverage 
        // apply filter if needed 
        let (verdi_feedback_tgl, verdi_observer_tgl) = {
            let verdi_observer = unsafe {
                VerdiShMapObserver::<{ MAP_SIZE / 4 }>::from_mut_ptr(
                    "verdi_tgl",
                    workdir,
                    shmem_ptr,
                    &VerdiCoverageMetric::Toggle,
                    &"".to_string()
                )
            };

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_tgl", MAP_SIZE, workdir);

            (feedback, verdi_observer)
        };

        let mut shmem = shmem_provider_client.new_shmem(MAP_SIZE).unwrap();
        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
        let (verdi_feedback_fsm, verdi_observer_fsm) = {
            let verdi_observer = unsafe {
                VerdiShMapObserver::<{ MAP_SIZE / 4 }>::from_mut_ptr(
                    "verdi_fsm",
                    workdir,
                    shmem_ptr,
                    &VerdiCoverageMetric::FSM,
                    &"".to_string()
                )
            };

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_fsm", MAP_SIZE, workdir);

            (feedback, verdi_observer)
        };

        let mut shmem = shmem_provider_client.new_shmem(MAP_SIZE).unwrap();
        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
        let (verdi_feedback_condition, verdi_observer_condition) = {
            let verdi_observer = unsafe {
                VerdiShMapObserver::<{ MAP_SIZE / 4 }>::from_mut_ptr(
                    "verdi_condition",
                    workdir,
                    shmem_ptr,
                    &VerdiCoverageMetric::Condition,
                    &"".to_string()
                )
            };

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_condition", MAP_SIZE, workdir);

            (feedback, verdi_observer)
        };

        let mut shmem = shmem_provider_client.new_shmem(MAP_SIZE).unwrap();
        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
        let (verdi_feedback_line, verdi_observer_line) = {
            let verdi_observer = unsafe {
                VerdiShMapObserver::<{ MAP_SIZE / 4 }>::from_mut_ptr(
                    "verdi_line",
                    workdir,
                    shmem_ptr,
                    &VerdiCoverageMetric::Line,
                    &"".to_string()
                )
            };

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_line", MAP_SIZE, workdir);

            (feedback, verdi_observer)
        };


        let mut shmem = shmem_provider_client.new_shmem(MAP_SIZE).unwrap();
        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
        let (verdi_feedback_branch, verdi_observer_branch) = {
            let verdi_observer = unsafe {
                VerdiShMapObserver::<{ MAP_SIZE / 4 }>::from_mut_ptr(
                    "verdi_branch",
                    workdir,
                    shmem_ptr,
                    &VerdiCoverageMetric::Branch,
                    &"".to_string()
                )
            };

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_branch", MAP_SIZE, workdir);

            (feedback, verdi_observer)
        };

        let mut feedback = feedback_or!(verdi_feedback_line, verdi_feedback_tgl, verdi_feedback_fsm, verdi_feedback_branch, verdi_feedback_condition);

        let mut objective = feedback_not!(TransferredFeedback);

        // Instantiate State with feedback, objective, in/out corpus
        let mut state = StdState::new(
            StdRand::with_seed(current_nanos()),
            InMemoryCorpus::<BytesInput>::new(),
            InMemoryCorpus::new(),
            &mut feedback,
            &mut objective,
       )
        .unwrap();
        state.set_max_size(max_testcase_size);
       
        // Simle FIFO scheduler
        let scheduler = QueueScheduler::new();

        // RISCV ISA mutator
        let mutator = StdISAScheduledMutator::with_max_stack_pow(riscv_mutations(), 2);

        // Finally, instantiate the fuzzer
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
  
        let mut executor = simv.into_executor(tuple_list!(verdi_observer_line, verdi_observer_tgl, verdi_observer_fsm, verdi_observer_branch, verdi_observer_condition));


        let corpus_dir = PathBuf::from(corpus_dir.to_string());

        // load initial inputs if any seeds provided
        state
            .load_initial_inputs(&mut fuzzer, &mut executor, &mut mgr, &[corpus_dir.clone()]).unwrap();
        
        // Instantiate a mutational stage that will apply mutations to the selected testcase
        let sync_dir = PathBuf::from(sync_dir.to_string());
        let mut stages = tuple_list!(SyncFromDiskStage::new(sync_dir), StdMutationalStage::with_max_iterations(mutator, 1));

        fuzzer.fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in fuzzing loop");

        Ok(())
    };

    match Launcher::<_,_,_,BytesInput>::builder()
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
