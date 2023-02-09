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
use libafl::monitors::Monitor;
use libafl::monitors::UserStats;
use core::time::Duration;
use libafl::prelude::format_duration_hms;
use libafl::prelude::ClientStats;
use libafl::prelude::current_time;

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
    corpus::{OnDiskCorpus, InMemoryCorpus},
    inputs::bytes::BytesInput,
};
use libafl::executors::command::CommandConfigurator;
use libafl::prelude::MaxMapFeedback;

use libverdi::verdi_feedback::VerdiFeedback as VerdiFeedback;
use libverdi::verdi_observer::VerdiMapObserver as VerdiObserver;
use libverdi::verdi_observer::VerdiCoverageMetric;

mod vcs_executor;

#[cfg(feature = "std")]
/// Tracking monitor during fuzzing that just prints to `stdout`.
#[derive(Debug, Clone, Default)]
pub struct VCSMonitor {
    start_time: Duration,
    client_stats: Vec<ClientStats>,
}

#[cfg(feature = "std")]
impl VCSMonitor {
    /// Create a new [`VCSMonitor`]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "std")]
impl Monitor for VCSMonitor {
    /// the client monitor, mutable
    fn client_stats_mut(&mut self) -> &mut Vec<ClientStats> {
        &mut self.client_stats
    }

    /// the client monitor
    fn client_stats(&self) -> &[ClientStats] {
        &self.client_stats
    }

    /// Time this fuzzing run stated
    fn start_time(&mut self) -> Duration {
        self.start_time
    }

    fn display(&mut self, event_msg: String, sender_id: u32) {
        println!(
            "[{} #{}] run time: {}, clients: {}, corpus: {}, objectives: {}, executions: {}, exec/sec: {}",
            event_msg,
            sender_id,
            format_duration_hms(&(current_time() - self.start_time)),
            self.client_stats().len(),
            self.corpus_size(),
            self.objective_size(),
            self.total_execs(),
            self.execs_per_sec(),
            // self.execs_per_sec_pretty(),
        );

        let mut id = 0;
        for client in self.client_stats_mut() {
            let coverage = client.get_user_stats("coverage").unwrap();
            println!("Client {} -> {} % coverage score.", id, coverage);
            id += 1;
        }
        println!();

        // Only print perf monitor if the feature is enabled
        #[cfg(feature = "introspection")]
        {
            // Print the client performance monitor.
            println!(
                "Client {:03}:\n{}",
                sender_id, self.client_stats[sender_id as usize].introspection_monitor
            );
            // Separate the spacing just a bit
            println!();
        }
    }
}

#[allow(clippy::similar_names)]
pub fn main() {

    let res = clap_cmd::new("forkserver_simple")
        .about("Example Forkserver fuzer")
        .arg(
            Arg::new("executable")
                .help("The instrumented binary we want to fuzz")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("corpus")
                .help("The directory to read initial inputs from ('seeds')")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("vdb")
                .help("The path to the vdb directory")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("outdir")
                .help("The directory to write the outputs to")
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

    // const MAP_SIZE: usize = 65536;
    // let mut shmem_provider = unix_shmem::MmapShMemProvider::new().unwrap();
    // The coverage map shared between observer and executor
    // The shared memory id is saved in __AFL_SHM_ID 
    // let mut shmem = shmem_provider.new_shmem(MAP_SIZE).unwrap();
    // shmem.write_to_env("__AFL_SHM_ID").unwrap();
    // let shmem_id = shmem.id();
    // let shmem_buf = shmem.as_mut_slice();

    // Load user provided parameters
    let executable = res.value_of("executable").unwrap().to_string();
    let args = res.value_of("arguments").unwrap().to_string();
    let vdb = res.value_of("vdb").unwrap().to_string();
    
    let mut outdir = res.value_of("outdir").unwrap().to_string();
    outdir.push_str("/solutions");
    let solution_dir = PathBuf::from(outdir);

    // The vcs observer observes the coverage metrics exposed by vcs
    // The coverage is for now collected by another process since the lib is in c++
    // But would be interesting to get everything at one place in the future
    let map_size: usize = 42000;

    let outdir = res.value_of("outdir").unwrap().to_string();
    let verdi_observer = VerdiObserver::new("verdi_map", &outdir, map_size, &VerdiCoverageMetric::toggle);

    let map_feedback = MaxMapFeedback::new_tracking(&verdi_observer, true, false);

    // Feedback to rate the interestingness of an input
    // This one is composed by two Feedbacks in OR
    let outdir = res.value_of("outdir").unwrap().to_string();
    let mut feedback = feedback_or!(
        VerdiFeedback::new_with_observer("verdi_map", map_size, &outdir).
        map_feedback
    );

    // A feedback to choose if an input is a solution or not
    // We want to do the same crash deduplication that AFL does
    let mut objective = ();

    // If not restarting, create a State from scratch
    let corpus_dir = PathBuf::from(res.value_of("corpus").unwrap().to_string());
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

    // The Monitor trait define how the fuzzer stats are displayed to the user
    let mon = VCSMonitor::new();

    // The event manager handle the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(mon);

    // A queue policy to get testcasess from the corpus
    let scheduler = QueueScheduler::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    // let executable = res.value_of("executable").unwrap();
    let outdir = res.value_of("outdir").unwrap().to_string();
    let mut executor = vcs_executor::VCSExecutor { executable, args, outdir}.into_executor(tuple_list!(verdi_observer));

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

