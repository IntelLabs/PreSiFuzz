// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

// TODO: Launch multiple fuzzer instance in parallel 
//! The [`Launcher`] launches multiple fuzzer instances in parallel.
//! Thanks to it, we won't need a `for` loop in a shell script...
//!
//! It will hide child output, unless the settings indicate otherwise, or the `LIBAFL_DEBUG_OUTPUT` env variable is set.
//!
//! To use multiple [`Launcher`]`s` for individual configurations,
//! we can set `spawn_broker` to `false` on all but one.
//!
//! To connect multiple nodes together via TCP, we can use the `remote_broker_addr`.
//! (this requires the `llmp_bind_public` compile-time feature for `LibAFL`).
//!
//! On `Unix` systems, the [`Launcher`] will use `fork` if the `fork` feature is used for `LibAFL`.
//! Else, it will start subsequent nodes with the same commandline, and will set special `env` variables accordingly.

#[cfg(feature = "std")]
use core::marker::PhantomData;
use core::{
    fmt::{self, Debug, Formatter},
};

#[cfg(feature = "std")]
use libafl::{
    events::{
        EventConfig,
    },
    monitors::Monitor,
    state::{HasExecutions, State, HasCorpus},
    inputs::{Input},
    Error,
};
#[cfg(feature = "std")]
use typed_builder::TypedBuilder;

use crate::manager::SyncOnDiskRestartingMgr;
use crate::manager::SyncOnDiskManagerKind;
use crate::manager::SyncOnDiskRestartingEventManager; 

/// The (internal) `env` that indicates we're running as client.
const _AFL_LAUNCHER_CLIENT: &str = "AFL_LAUNCHER_CLIENT";

/// Provides a [`Launcher`], which can be used to launch a fuzzing run on a specified list of cores
///
/// Will hide child output, unless the settings indicate otherwise, or the `LIBAFL_DEBUG_OUTPUT` env variable is set.
#[cfg(feature = "std")]
#[allow(
    clippy::type_complexity,
    missing_debug_implementations,
    clippy::ignored_unit_patterns
)]
#[derive(TypedBuilder)]
pub struct Launcher<'a, CF, MT, S, I>
where
    CF: FnOnce(Option<S>, SyncOnDiskRestartingEventManager<S, I>, u32) -> Result<(), Error>,
    S::Input: 'a,
    MT: Monitor,
    S: State + 'a + HasCorpus,
    I: Input,
{
    /// The monitor instance to use
    monitor: MT,
    /// The configuration
    configuration: EventConfig,
    #[builder(default = "/tmp/corpus".to_string())]
    sync_dir: String,
    /// The 'main' function to run for each client forked. This probably shouldn't return
    #[builder(default, setter(strip_option))]
    run_client: Option<CF>,
    /// A file name to write all client output to
    #[builder(default = None)]
    stdout_file: Option<&'a str>,
    /// A file name to write all client stderr output to. If not specified, output is sent to
    /// `stdout_file`.
    #[builder(default = None)]
    stderr_file: Option<&'a str>,
    #[builder(setter(skip), default = PhantomData)]
    phantom_data: PhantomData<(&'a S, I)>,
}

impl<CF, MT, S, I> Debug for Launcher<'_, CF, MT, S, I>
where
    CF: FnOnce(Option<S>, SyncOnDiskRestartingEventManager<S, I>, u32) -> Result<(), Error>,
    MT: Monitor + Clone,
    S: State + HasCorpus,
    I: Input,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Launcher")
            .field("configuration", &self.configuration)
            .field("stdout_file", &self.stdout_file)
            .field("stderr_file", &self.stderr_file)
            .field("sync_dir", &self.sync_dir)
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "std")]
impl<'a, CF, MT, S, I> Launcher<'a, CF, MT, S, I>
where
    CF: FnOnce(Option<S>, SyncOnDiskRestartingEventManager<S, I>, u32) -> Result<(), Error>,
    MT: Monitor + Clone,
    S: State + HasExecutions + HasCorpus,
    I: Input,
{

    /// Launch the broker and the clients and fuzz
    #[cfg(all(feature = "std", any(windows, not(feature = "fork"))))]
    #[allow(unused_mut, clippy::match_wild_err_arm)]
    pub fn launch(&mut self) -> Result<(), Error> {

        let is_client = std::env::var(_AFL_LAUNCHER_CLIENT);

        match is_client {
            Ok(core_conf) => {
                let core_id = core_conf.parse()?;
     
                println!("I am client!! with id {}", core_id);

                // the actual client. do the fuzzing
                let (state, mgr) = SyncOnDiskRestartingMgr::<MT, S, I>::builder()
                    .kind(SyncOnDiskManagerKind::Client {
                        cpu_core: Some(core_id),
                    })
                    .sync_dir(self.sync_dir.clone())
                    .configuration(self.configuration)
                    .build()
                    .launch()?;
            
                println!("Ready to fuzz!");

                return (self.run_client.take().unwrap())(state, mgr, core_id);

            }
            Err(std::env::VarError::NotPresent) => {
                // I am a broker
                // before going to the broker loop, spawn n clients
                println!("I am broker!!.");

                SyncOnDiskRestartingMgr::<MT, S, I>::builder()
                    .monitor(Some(self.monitor.clone()))
                    .kind(SyncOnDiskManagerKind::Broker)
                    .configuration(self.configuration)
                    .sync_dir(self.sync_dir.clone())
                    .build()
                    .launch()?;
            }
            Err(_) => panic!("Env variables are broken, received non-unicode!"),
        };
        
        Ok(())
    }
}

