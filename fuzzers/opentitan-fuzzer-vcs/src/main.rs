// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    path::PathBuf,
};

#[cfg(feature = "tui")]
use libafl::monitors::tui::TuiMonitor;

use libafl::prelude::MultiMonitor;
use libafl::inputs::HasTargetBytes;
use core::time::Duration;
use std::{
    process::{Stdio},
};
use libafl::prelude::Input;
use std::process::Command as pcmd;
use wait_timeout::ChildExt;

#[cfg(target_vendor = "apple")]
use libafl_bolts::shmem::UnixShMemProvider;
use libafl_bolts::{
        current_nanos,
        rands::StdRand,
        tuples::tuple_list,
        AsSlice};

#[cfg(not(feature = "tui"))]
use libafl::{
    executors::{inprocess::InProcessExecutor, ExitKind, TimeoutExecutor},
    events::{EventConfig},
    fuzzer::{Fuzzer, StdFuzzer},
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    schedulers::QueueScheduler,
    stages::mutational::StdMutationalStage,
    state::StdState,
    corpus::{OnDiskCorpus, InMemoryCorpus},
    inputs::bytes::BytesInput,
    Error
};
use std::env;
use std::path::Path;
use std::io::prelude::*;
use std::fs::File;
use tempdir::TempDir;


#[cfg(libnpi)]
use libpresifuzz_feedbacks::verdi_feedback::VerdiFeedback;
#[cfg(libnpi)]
use libpresifuzz_observers::verdi_observer::VerdiShMapObserver;
#[cfg(libnpi)]
use libpresifuzz_observers::verdi_observer::VerdiCoverageMetric;

#[cfg(not(libnpi))]
use libpresifuzz_feedbacks::verdi_xml_feedback::VerdiFeedback;
#[cfg(not(libnpi))]
use libpresifuzz_observers::verdi_xml_observer::VerdiXMLMapObserver;
#[cfg(not(libnpi))]
use libpresifuzz_observers::verdi_xml_observer::VerdiCoverageMetric;
#[cfg(not(libnpi))]
use libpresifuzz_observers::verdi_xml_observer::VerdiCoverageMetric::*;

use libpresifuzz_ec::llmp::Launcher;
use libpresifuzz_stages::sync::SyncFromDiskStage;

#[cfg(not(libnpi))]
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

// mod vcs_executor;

#[derive(Debug)]
pub struct WorkDir(Option<TempDir>);

// Forward inherent methods to the tempdir crate.
use std::io::Result;
impl WorkDir {
    pub fn new(prefix: &str) -> Result<WorkDir>
    { TempDir::new(prefix).map(Some).map(WorkDir) }

    pub fn path(&self) -> &Path
    { self.0.as_ref().unwrap().path() }
}

/// Leaks the inner TempDir if we are unwinding.
impl Drop for WorkDir {
    fn drop(&mut self) {
        ::std::mem::forget(self.0.take())
    }
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn simv_spawn_child<I: Input + HasTargetBytes>(input: &I, _workdir: String, executable: String, args: String) -> Result<std::process::Child> { 
    let do_steps = || -> Result<()> {

        let mut file = File::create("testcase")?;
        let hex_input = input.target_bytes();
        let hex_input2 = hex_input.as_slice();
        for i in 0..hex_input2.len()-1 {
            let c: char = hex_input2[i].try_into().unwrap();
            write!(file, "{}", c as char)?;
        }
        Ok(())
    };

    if let Err(_err) = do_steps() {
        println!("VCSExecutor failed to create new input file, please check output argument");
    }

    let mut command = pcmd::new(executable.as_str());
    
    let args_vec: Vec<&str> = args.as_str().split(' ').collect();
    let args_v = &args_vec[0 .. args_vec.len()];

    let command = command.args(args_v);

    let command = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let child = command.spawn().expect("failed to start process");

    Ok(child)
}


#[allow(clippy::similar_names)]
pub fn main() {
    
    cfg_if::cfg_if! {
        if #[cfg(libnpi)] {

        const MAP_SIZE: usize = 65536*4;
        //Coverage map shared between observer and executor
        #[cfg(target_vendor = "apple")]
        let mut shmem_provider = UnixShMemProvider::new().unwrap();
        #[cfg(not(target_vendor = "apple"))]
        let mut shmem_provider = StdShMemProvider::new().unwrap();
        let mut shmem = shmem_provider.new_shmem(MAP_SIZE).unwrap();
        }
    }
    
    let sync_dir = format!("{}/sync/", std::env::current_dir().unwrap().display());
    println!("sync_dir: {}", sync_dir);

    let mon = MultiMonitor::new(|s| println!("{s}"));
    // let mon = SimpleMonitor::new(|s| println!("{}", s));
    // let mut mgr = SimpleEventManager::new(mon);

    let mut run_client = |_state: Option<_>, mut mgr, _core_id| {

        let simv_name = "./lowrisc_ip_aes_0.6".to_string();
        let simv_args = "+TESTCASE=testcase -cm tgl".to_string();
        let vdb_name = "Coverage.vdb".to_string();
        let simv_dir = "build".to_string();
        let solution_dir = "solutions".to_string();
        let _seeds_dir = "seeds".to_string();

        let dir = env::temp_dir();
        println!("Temporary directory: {}. Please, customize TMPDIR if needed.", dir.display());
        
        // get a unique temp-dir name
        let tmp_dir = WorkDir::new("presifuzz_").expect("Unable to create temporary directory");
        let workdir = tmp_dir.path().as_os_str().to_str().unwrap().to_string();
        let seeds_dir = "seeds";

        println!("PreSiFuzz v0.2 \n
                 Author: Nassim Corteggiani \n
                 Contact: nassim.corteggiani@intel.com \n
                 \n\
                 Environment Assumptions:         \n \
                    * workdir: {}                 \n \
                    * vdb name: {}                \n \
                    * simv name: {}               \n \
                    * simv args: {}               \n \
                    * original simv directory: {} \n \
                    * solution directory: {}      \n \
                    * seeds directory: {}      \n \
                ", workdir, vdb_name, simv_name, simv_args, simv_dir, solution_dir, seeds_dir);
       
        let src = simv_dir.clone();
        let dst = workdir.clone();
        copy_dir_all(&Path::new(&src), &Path::new(&dst))?;

        let mut dst = workdir.clone();
        dst.push_str("/seeds");
        copy_dir_all(&seeds_dir, &Path::new(&dst))?;

        cfg_if::cfg_if! {
            if #[cfg(libnpi)] {
                let mut src = workdir.clone();
                src.push_str(&format!("/{}", vdb_name));
                let mut dst = workdir.clone();
                dst.push_str("/Virgin_coverage.vdb");
                copy_dir_all(Path::new(&src), Path::new(&dst))?;
            }
        }
        
        std::env::set_current_dir(&workdir).expect("Unable to change into {dir}");

        cfg_if::cfg_if! {
            if #[cfg(libnpi)] {
                let shmem_buf = shmem.as_mut_slice();
                let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;
            }
        }

        let (mut feedback, verdi_observer) = 
        {

            cfg_if::cfg_if! {

                if #[cfg(libnpi)] {
        
                    const MAP_SIZE: usize = 38542;

                    println!("Coverage collected using VERDI libNPI");
                    let verdi_observer = unsafe{VerdiShMapObserver::<{MAP_SIZE/4}>::from_mut_ptr("verdi_map", &workdir, shmem_ptr, &VerdiCoverageMetric::Toggle, &"".to_string())};

                    let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_map", MAP_SIZE, &workdir);

                } else {

                    println!("Coverage collected using xml parsers");
                    create_verdi_observer_and_feedback!(
                        verdi_observer_tgl,
                        verdi_feedback_tgl,
                        Toggle,
                        false,
                        workdir
                    );

                    (verdi_feedback_tgl, verdi_observer_tgl)
                }
            }
        };


        // A feedback to choose if an input is a solution or not
        // We want to do the same crash deduplication that AFL does
        let mut objective = ();

        // If not restarting, create a State from scratch
        let mut state = StdState::new(
            // RNG
            StdRand::with_seed(current_nanos()),
            // Use on disk corpus, so that we keep trace of it
            // Performance impact is negligeable as the target is way slower
            InMemoryCorpus::<BytesInput>::new(),
            // Corpus in which we store solutions (crashes in this example),
            // on disk so the user can get them after stopping the fuzzer
            OnDiskCorpus::new(&solution_dir).unwrap(),
            // States of the feedbacks.
            // The feedbacks can report the data that should persist in the State.
            &mut feedback,
            // Same for objective feedbacks
            &mut objective,
        )
        .unwrap();

        // A queue policy to get testcasess from the corpus
        let scheduler = QueueScheduler::new();

        // A fuzzer with feedbacks and a corpus scheduler
        let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
        
        let mut simv_harness = |input: &BytesInput| {

            let mut child = simv_spawn_child(input, workdir.clone(), simv_name.clone(), simv_args.clone()).expect("Unable to start simv!");

            match child
                .wait_timeout(Duration::from_secs(20))
                .expect("waiting on child failed")
                .map(|status| status.unix_signal())
            {
                Some(Some(9)) => {
                    return ExitKind::Oom
                },
                Some(Some(_)) => {
                    return ExitKind::Crash
                },
                Some(None) => {
                    return ExitKind::Ok
                },
                None => {
                    drop(child.kill());
                    drop(child.wait());
                    return ExitKind::Timeout
                }
            };
        };

        // Switch to InProcessExecutor for LLMP
        // let mut executor = vcs_executor::VCSExecutor { executable: simv_name, args:simv_args, workdir: workdir}.into_executor(tuple_list!(verdi_observer));
        let mut executor = TimeoutExecutor::new(
            InProcessExecutor::new(
                            &mut simv_harness,
                            tuple_list!(verdi_observer),
                            &mut fuzzer,
                            &mut state,
                            &mut mgr,
            )?,
            Duration::from_millis(20000),
        );

        // Load initial inputs from corpus
        let corpus_dir = PathBuf::from(seeds_dir);
        state
            .load_initial_inputs(&mut fuzzer, &mut executor, &mut mgr, &[corpus_dir])
            .expect("Failed to load the initial corpus");

        // Setup a mutational stage with a basic bytes mutator
        let mutator = StdScheduledMutator::new(havoc_mutations());
        
        // Instantiate a mutational stage that will apply mutations to the selected testcase
        let sync_dir = PathBuf::from(sync_dir.to_string());
        let mut stages = tuple_list!(
            SyncFromDiskStage::new(sync_dir),
            StdMutationalStage::new(mutator)
        );

        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in the fuzzing loop");

        Ok(())
    };

    cfg_if::cfg_if! {
        if #[cfg(libnpi)] {
            match Launcher::<_, _, _, BytesInput>::builder()
                .configuration(EventConfig::from_name("default"))
                .shmem_provider(shmem_provider)
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
        } else {
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
    }

}

