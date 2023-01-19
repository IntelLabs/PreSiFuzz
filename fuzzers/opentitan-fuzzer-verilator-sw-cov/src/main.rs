// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    path::PathBuf,
};

use clap::Arg;
use clap::Command as clap_cmd;

#[cfg(feature = "tui")]
use libafl::monitors::tui::TuiMonitor;
#[cfg(not(feature = "tui"))]
use libafl::{
    feedback_and_fast, feedback_or,
    bolts::{
        current_nanos,
        rands::StdRand,
        tuples::tuple_list,
    },
    events::SimpleEventManager,
    feedbacks::{CrashFeedback, TimeFeedback},
    fuzzer::{Fuzzer, StdFuzzer},
    monitors::SimpleMonitor,
    observers::{TimeObserver},
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    schedulers::QueueScheduler,
    stages::mutational::StdMutationalStage,
    state::StdState,
    corpus::{OnDiskCorpus},
    inputs::bytes::BytesInput,
};
use libafl::executors::command::CommandConfigurator;
use libafl::prelude::HitcountsMapObserver;
use libafl::prelude::ConstMapObserver;
use libafl::prelude::MaxMapFeedback;
use libafl::prelude::ShMemId;
use libafl::prelude::Input;
use libafl::prelude::HasTargetBytes;
use std::process::Child;
use std::process::Stdio;
use std::path::Path;
use libafl::bolts::AsSlice;
use std::process::Command;
use std::io::Write;
use libafl::bolts::shmem::UnixShMemProvider;
use libafl::prelude::ShMemProvider;
use libafl::bolts::AsMutSlice;
use libafl::prelude::ShMem;
use libafl::Error;

mod vcs_executor;

#[allow(clippy::similar_names)]
pub fn main() {

    let res = clap_cmd::new("forkserver_simple")
        .about("Example Forkserver fuzer")
        .arg(
            Arg::new("corpus")
                .help("The directory to read initial inputs from ('seeds')")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("timeout")
                .help("Timeout for each individual execution, in milliseconds")
                .short('t')
                .long("timeout")
                .default_value("1200"),
        )
        .arg(
            Arg::new("debug_child")
                .help("If not set, the child's stdout and stderror will be redirected to /dev/null")
                .short('d')
                .long("debug-child"),
        )
        .arg(
            Arg::new("arguments")
                .help("Arguments passed to the target")
                .multiple_values(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("signal")
                .help("Signal used to stop child")
                .short('s')
                .long("signal")
                .default_value("SIGKILL"),
        )
        .get_matches();

    const MAP_SIZE: usize = 65536;
    //Coverage map shared between observer and executor
    let mut shmem_provider = UnixShMemProvider::new().unwrap();
    // let mut shmem_provider = unix_shmem::MmapShMemProvider::new().unwrap();
    // The coverage map shared between observer and executor
    // The shared memory id is saved in __AFL_SHM_ID 
    let mut shmem = shmem_provider.new_shmem(MAP_SIZE).unwrap();
    shmem.write_to_env("__AFL_SHM_ID").unwrap();
    let shmem_id = shmem.id();
    let shmem_map = shmem.as_mut_slice();

    let solution_dir = PathBuf::from("./solutions");

    // The vcs observer observes the coverage metrics exposed by vcs
    // The coverage is for now collected by another process since the lib is in c++
    // But would be interesting to get everything at one place in the future
    let map_size: usize = 42000;

    // Create an observation channel using the signals map
    let edges_observer = HitcountsMapObserver::new(ConstMapObserver::<_, MAP_SIZE>::new(
        "shared_mem",
        shmem_map
    ));


    // Create an observation channel to keep track of the execution time
    let time_observer = TimeObserver::new("time");

    // Feedback to rate the interestingness of an input
    // This one is composed by two Feedbacks in OR
    let mut feedback = MaxMapFeedback::new_tracking(&edges_observer, true, false);

    // A feedback to choose if an input is a solution or not
    // We want to do the same crash deduplication that AFL does
    let mut objective = feedback_and_fast!(
        // When an assertion failed, we could trigger a POSIX signal value
        // that mimics a crash.
        CrashFeedback::new(),
        // Take it only if trigger new coverage over crashes
        // This will discard redondant findings
        CrashFeedback::new()
    );

    // If not restarting, create a State from scratch
    let corpus_dir = PathBuf::from(res.value_of("corpus").unwrap().to_string());
    let mut state = StdState::new(
        // RNG
        StdRand::with_seed(current_nanos()),
        // Use on disk corpus, so that we keep trace of it
        // Performance impact is negligeable as the target is way slower
        OnDiskCorpus::<BytesInput>::new(corpus_dir).unwrap(),
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

    // The Monitor trait define how the fuzzer stats are displayed to the user
    let mon = SimpleMonitor::new(|s| println!("{}", s));

    // The event manager handle the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(mon);

    // A queue policy to get testcasess from the corpus
    let scheduler = QueueScheduler::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    // Create the executor for an in-process function with just one observer
    #[derive(Debug)]
    struct MyExecutor {
        shmem_id: ShMemId,
    }

    impl CommandConfigurator for MyExecutor {
        fn spawn_child<I: Input + HasTargetBytes>(&mut self, input: &I) -> Result<Child, Error> {

            let mut command = Command::new("./bin/aes");

            let command = command
                // .args(&[self.shmem_id.as_str()])
                .env_clear()
                .env("__AFL_SHM_ID", self.shmem_id.as_str())
                .stdin(Stdio::piped());
                // .stdout(Stdio::piped());

            let mut child = command.spawn().expect("failed to start process");
           
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(input.target_bytes().as_slice())?;

            
            // let output = command.output().expect("failed to start process");
            // println!("status: {}", String::from_utf8_lossy(&output.stdout));

            Ok(child)
        }
    }

    let mut executor = MyExecutor { shmem_id }.into_executor(tuple_list!(edges_observer));

    // Load initial inputs from corpus
    let corpus_dir = PathBuf::from(res.value_of("corpus").unwrap().to_string());
    state
        .load_initial_inputs(&mut fuzzer, &mut executor, &mut mgr, &[corpus_dir])
        .expect("Failed to load the initial corpus");

    // Setup a mutational stage with a basic bytes mutator
    let mutator = StdScheduledMutator::new(havoc_mutations());
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Error in the fuzzing loop");

}

