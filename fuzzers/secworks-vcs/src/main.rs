// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
// Based on libAFL 0.8.1 examples https://github.com/AFLplusplus/LibAFL "Andrea Fioraldi <andreafioraldi@gmail.com>", "Dominik Maier <domenukk@gmail.com>" (Apache 2.0)

use std::{
    path::PathBuf,
};

#[cfg(not(target_vendor = "apple"))]
use libafl::bolts::shmem::StdShMemProvider;
#[cfg(target_vendor = "apple")]
use libafl::bolts::shmem::UnixShMemProvider;

#[cfg(feature = "tui")]
use libafl::monitors::tui::TuiMonitor;
#[cfg(not(feature = "tui"))]
use libafl::{
    bolts::{
        core_affinity::Cores,
        current_nanos,
        rands::StdRand,
        shmem::{ShMemProvider},
        tuples::tuple_list,
        AsMutSlice
    },
    executors::{inprocess::InProcessExecutor, ExitKind, TimeoutExecutor},
    events::{EventConfig},
    fuzzer::{Fuzzer, StdFuzzer},
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    schedulers::QueueScheduler,
    stages::mutational::StdMutationalStage,
    state::StdState,
    corpus::{InMemoryCorpus, OnDiskCorpus},
    inputs::bytes::BytesInput,
    Error
};
use core::time::Duration;
use std::path::Path;
use libafl::inputs::HasTargetBytes;
use libafl::prelude::Input;
use std::fs::File;
use libafl::bolts::AsSlice;
use std::io::Write;
use libafl::prelude::MultiMonitor;
use libafl::prelude::Launcher;
use std::{
    process::{Stdio},
};
use std::process::Command as pcmd;

use libafl_verdi::verdi_feedback::VerdiFeedback;
use libafl_verdi::verdi_observer::VerdiShMapObserver;
use libafl_verdi::verdi_observer::VerdiCoverageMetric;
use tempdir::TempDir;
use std::env;

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
    
    let cores = Cores::all().unwrap();

    const MAP_SIZE: usize = 65536*4;

    //Coverage map shared between observer and executor
    #[cfg(target_vendor = "apple")]
    let mut shmem_provider = UnixShMemProvider::new().unwrap();
    #[cfg(not(target_vendor = "apple"))]
    let mut shmem_provider = StdShMemProvider::new().unwrap();
    let mut shmem = shmem_provider.new_shmem(MAP_SIZE).unwrap();
    
    let mon = MultiMonitor::new(|s| println!("{s}"));
    // let mon = SimpleMonitor::new(|s| println!("{}", s));
    // let mut mgr = SimpleEventManager::new(mon);
    
    let mut run_client = |_state: Option<_>, mut mgr, _core_id| {
   
        let simv_name = "./secworks_crypto_sha256_0".to_string();
        let simv_args = "+TESTCASE=testcase -cm tgl".to_string();
        let vdb_name = "Coverage.vdb".to_string();
        let simv_dir = "./build/secworks_crypto_sha256_0/tb_fuzz-vcs".to_string();
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
       
        let dst = workdir.clone();
        copy_dir_all(Path::new(&simv_dir), Path::new(&dst))?;

        let mut dst = workdir.clone();
        dst.push_str("/seeds");
        copy_dir_all(seeds_dir, Path::new(&dst))?;

        let mut src = workdir.clone();
        src.push_str(&format!("/{}", vdb_name));
        let mut dst = workdir.clone();
        dst.push_str("/Virgin_coverage.vdb");
        copy_dir_all(Path::new(&src), Path::new(&dst))?;
        
        std::env::set_current_dir(&workdir).expect("Unable to change into {dir}");


        let shmem_buf = shmem.as_mut_slice();
        let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;

        let (mut feedback, verdi_observer) = 
        {
            let verdi_observer = unsafe{VerdiShMapObserver::<{MAP_SIZE/4}>::from_mut_ptr("verdi_map", &workdir, shmem_ptr, &VerdiCoverageMetric::Toggle, &"".to_string())};

            let feedback = VerdiFeedback::<{MAP_SIZE/4}>::new_with_observer("verdi_map", MAP_SIZE, &workdir);
            // let feedback = MaxMapFeedback::new(&verdi_observer);

            (feedback, verdi_observer)
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

            use wait_timeout::ChildExt;
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
        let mut stages = tuple_list!(StdMutationalStage::new(mutator));

        fuzzer
            .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
            .expect("Error in the fuzzing loop");
        Ok(())
    };

    match Launcher::builder()
        .configuration(EventConfig::AlwaysUnique)
        .shmem_provider(shmem_provider)
        .monitor(mon)
        .run_client(&mut run_client)
        .cores(&cores)
        .broker_port(1337)
        .stdout_file(Some("/dev/null"))
        .build()
        .launch()
    {
        Ok(()) => (),
        Err(Error::ShuttingDown) => (),
        Err(err) => panic!("Fuzzingg failed {err:?}"),
    };

}

