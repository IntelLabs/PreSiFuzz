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
use core::time::Duration;
use libafl::prelude::format_duration_hms;
use libafl::prelude::ClientStats;
use libafl::prelude::current_time;

#[cfg(not(target_vendor = "apple"))]
use libafl::bolts::shmem::StdShMemProvider;
#[cfg(target_vendor = "apple")]
use libafl::bolts::shmem::UnixShMemProvider;

#[cfg(not(feature = "tui"))]
use libafl::{
    bolts::{
        current_nanos,
        rands::StdRand,
        tuples::tuple_list,
        shmem::{ShMem, ShMemProvider},
        AsMutSlice
    },
    events::SimpleEventManager,
    fuzzer::{Fuzzer, StdFuzzer},
    mutators::scheduled::{havoc_mutations, StdScheduledMutator},
    schedulers::QueueScheduler,
    stages::mutational::StdMutationalStage,
    state::StdState,
    corpus::{OnDiskCorpus, InMemoryCorpus},
    inputs::bytes::BytesInput,
};
use std::env;
use std::path::Path;
use tempdir::TempDir;
use libafl::executors::command::CommandConfigurator;
// use libafl::prelude::MaxMapFeedback;

use libafl_verdi::verdi_feedback::VerdiFeedback as VerdiFeedback;
// use libafl_verdi::verdi_observer::VerdiMapObserver;
use libafl_verdi::verdi_observer::VerdiShMapObserver;
use libafl_verdi::verdi_observer::VerdiCoverageMetric;

mod vcs_executor;

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

    let simv_name = "./lowrisc_ip_aes_0.6".to_string();
    let mut simv_args = "+TESTCASE=testcase -cm tgl".to_string();
    let vdb_name = "Coverage.vdb".to_string();
    let simv_dir = "build/lowrisc_ip_aes_0.6/syn-vcs/".to_string();
    let solution_dir = "solutions".to_string();
    let seeds_dir = "seeds".to_string();

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
    
    let mut src = simv_dir.clone();
    let mut dst = workdir.clone();
    copy_dir_all(&Path::new(&src), &Path::new(&dst));
    
    let mut dst = workdir.clone();
    dst.push_str("/seeds");
    copy_dir_all(&seeds_dir, &Path::new(&dst));

    std::env::set_current_dir(&workdir).expect("Unable to change into {dir}");

    const MAP_SIZE: usize = 65536 * 4;

    #[cfg(target_vendor = "apple")]
    let mut shmem_provider = UnixShMemProvider::new().unwrap();
    #[cfg(not(target_vendor = "apple"))]
    let mut shmem_provider = StdShMemProvider::new().unwrap();
    let mut shmem = shmem_provider.new_shmem(MAP_SIZE).unwrap();
    shmem.write_to_env("__AFL_SHM_ID").unwrap();
    let shmem_buf = shmem.as_mut_slice();
    let shmem_ptr = shmem_buf.as_mut_ptr() as *mut u32;

    let (mut feedback, verdi_observer) = 
    {
        let verdi_observer = unsafe{VerdiShMapObserver::<{MAP_SIZE/4}>::from_mut_ptr("verdi_map", &workdir, shmem_ptr, &VerdiCoverageMetric::Toggle)};

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

    // The Monitor trait define how the fuzzer stats are displayed to the user
    let mon = VCSMonitor::new();

    // The event manager handle the various events generated during the fuzzing loop
    // such as the notification of the addition of a new item to the corpus
    let mut mgr = SimpleEventManager::new(mon);

    // A queue policy to get testcasess from the corpus
    let scheduler = QueueScheduler::new();

    // A fuzzer with feedbacks and a corpus scheduler
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = vcs_executor::VCSExecutor { executable: simv_name, args:simv_args, workdir: workdir}.into_executor(tuple_list!(verdi_observer));

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

}

