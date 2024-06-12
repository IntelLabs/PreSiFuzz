// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! NFS-backed event manager for scalable multi-processed fuzzing

use core::{
    marker::PhantomData,
};
use std::{
    env,
    io::{Read, Write},
};
use std::fs::File;
use libafl_bolts::{ClientId};
use serde::{de::DeserializeOwned, Deserialize};
#[cfg(feature = "std")]
use typed_builder::TypedBuilder;

use libafl::{
    events::{
        Event, BrokerEventResult, EventConfig, EventFirer, EventManager, EventManagerId,
        EventProcessor, EventRestarter, HasEventManagerId, ProgressReporter,
    },
    executors::{Executor, HasObservers},
    fuzzer::{EvaluatorObservers, ExecutionProcessor},
    inputs::{Input, UsesInput},
    monitors::Monitor,
    state::{HasExecutions, HasCorpus, HasLastReportTime, HasMetadata, State, UsesState},
    Error,
};
use libafl::prelude::ObserversTuple;
use serde::Serialize;
use std::{
    fs,
    time::SystemTime,
};
use std::thread;
use std::time::Duration;
use std::fs::OpenOptions;
use std::path::Path;

/// An TCP-backed event manager for simple multi-processed fuzzing
#[derive(Debug)]
pub struct SyncOnDiskEventBroker<I, MT>
where
    I: Input,
    MT: Monitor,
    //CE: CustomEvent<I>,
{
    sync_dir: String,
    monitor: MT,
    phantom: PhantomData<I>,
}

impl<I, MT> SyncOnDiskEventBroker<I, MT>
where
    I: Input,
    MT: Monitor,
{
    pub fn new(sync_dir: String, monitor: MT) -> Result<Self, Error> {
        Ok(Self{
            sync_dir: sync_dir,
            monitor: monitor,
            phantom: PhantomData,
        })
    }

    /// Run in the broker until all clients exit
    #[tokio::main(flavor = "current_thread")]
    #[allow(clippy::too_many_lines)]
    pub async fn broker_loop(&mut self) -> Result<(), Error> {
        
        let mut max_time = None;
        let mut last = None;
        let client_id = 0;

        loop {
            let in_dir = self.sync_dir.clone();
            for entry in fs::read_dir(in_dir)? {
                let entry = entry?;
                let path = entry.path();
                let attributes = fs::metadata(&path);

                if attributes.is_err() {
                    continue;
                }

                let attr = attributes?;

                if attr.is_file() && attr.len() > 0 {
                    if let Ok(time) = attr.modified() {
                        if let Some(l) = last {
                            if time.duration_since(l).is_err() {
                                continue;
                            }
                        }
                        max_time = Some(max_time.map_or(time, |t: SystemTime| t.max(time)));

                        // Read the file back and deserialize the Event struct
        
                        #[cfg(not(feature = "serialize_bytes"))]
                        {
                            let path_str = path.to_str().expect("Failed to convert path to string");
                            let mut file = File::open(path_str).expect("Failed to event file from corpus directory");
                            let mut serialized_event = String::new();
                            file.read_to_string(&mut serialized_event).expect("Failed to read file");

                            // Deserialize the JSON string back into an Event struct
                            let event: Event<I> = serde_json::from_str(&serialized_event).expect("Failed to deserialize event");
                            match Self::handle_in_broker(&mut self.monitor, ClientId(client_id), &event).unwrap() {
                                BrokerEventResult::Forward => {
                                    println!("Forwarding a new testcase");
                                }
                                BrokerEventResult::Handled => (),
                            };
                        }
                        
                        #[cfg(feature = "serialize_bytes")]
                        {
                            // Read the file back and deserialize the Event struct
                            let path_str = path.to_str().expect("Failed to convert path to string");
                            let mut file = File::open(path_str).expect("Failed to open file");
                            let mut serialized_event = Vec::new();
                            file.read_to_end(&mut serialized_event).expect("Failed to read file");

                            // Deserialize the binary data back into an Event struct
                            let event: Event<I> = bincode::deserialize(&serialized_event).expect("Failed to deserialize event");
                            match Self::handle_in_broker(&mut self.monitor, ClientId(client_id), &event).unwrap() {
                                BrokerEventResult::Forward => {
                                    println!("Forwarding a new testcase");
                                }
                                BrokerEventResult::Handled => (),
                            };
                        }
                    }
                }
            }
            last = max_time;

            // Wait for 30 seconds
            let duration = Duration::from_secs(30);
            println!("Waiting for 30 seconds...");
            thread::sleep(duration);
        }
    }

    /// Handle arriving events in the broker
    #[allow(clippy::unnecessary_wraps)]
    fn handle_in_broker(
        monitor: &mut MT,
        client_id: ClientId,
        event: &Event<I>,
    ) -> Result<BrokerEventResult, Error> {
        match &event {
            Event::NewTestcase {
                input: _,
                client_config: _,
                exit_kind: _,
                corpus_size,
                observers_buf: _,
                time,
                executions,
                forward_id,
            } => {
                let id = if let Some(id) = *forward_id {
                    id
                } else {
                    client_id
                };
                monitor.client_stats_insert(id);
                let client = monitor.client_stats_mut_for(id);
                client.update_corpus_size(*corpus_size as u64);
                client.update_executions(*executions as u64, *time);
                // monitor.display(event.name().to_string(), id);
                monitor.display(String::from("NewTestcase"), id);
                Ok(BrokerEventResult::Handled)
                // Ok(BrokerEventResult::Forward)
            }
            Event::UpdateExecStats {
                time,
                executions,
                phantom: _,
            } => {
                // TODO: The monitor buffer should be added on client add.
                monitor.client_stats_insert(client_id);
                let client = monitor.client_stats_mut_for(client_id);
                client.update_executions(*executions as u64, *time);
                // monitor.display(event.name().to_string(), client_id);
                // monitor.display(String::from("Execution Speed"), client_id);
                Ok(BrokerEventResult::Handled)
            }
            Event::UpdateUserStats {
                name,
                value,
                phantom: _,
            } => {
                monitor.client_stats_insert(client_id);
                let client = monitor.client_stats_mut_for(client_id);
                client.update_user_stats(name.clone(), value.clone());
                monitor.aggregate(name);
                // monitor.display(event.name().to_string(), client_id);
                // monitor.display(String::from("User Stats"), client_id);
                Ok(BrokerEventResult::Handled)
            }
            #[cfg(feature = "introspection")]
            Event::UpdatePerfMonitor {
                time,
                executions,
                introspection_monitor,
                phantom: _,
            } => {
                // TODO: The monitor buffer should be added on client add.

                // Get the client for the staterestorer ID
                monitor.client_stats_insert(client_id);
                let client = monitor.client_stats_mut_for(client_id);

                // Update the normal monitor for this client
                client.update_executions(*executions as u64, *time);

                // Update the performance monitor for this client
                client.update_introspection_monitor((**introspection_monitor).clone());

                // Display the monitor via `.display` only on core #1
                // monitor.display(event.name().to_string(), client_id);
                monitor.display(String::from("Perf"), client_id);

                // Correctly handled the event
                Ok(BrokerEventResult::Handled)
            }
            Event::Objective { objective_size } => {
                monitor.client_stats_insert(client_id);
                let client = monitor.client_stats_mut_for(client_id);
                client.update_objective_size(*objective_size as u64);
                // monitor.display(event.name().to_string(), client_id);
                monitor.display(String::from("Objective"), client_id);
                Ok(BrokerEventResult::Handled)
            }
            Event::Log {
                severity_level,
                message,
                phantom: _,
            } => {
                let (_, _) = (severity_level, message);
                // TODO rely on Monitor
                println!("{message}");
                Ok(BrokerEventResult::Handled)
            }
            Event::CustomBuf { .. } => Ok(BrokerEventResult::Forward),
            //_ => Ok(BrokerEventResult::Forward),
        }
    }
}

/// An [`EventManager`] that forwards all events to other attached via tcp.
pub struct SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    client_id: u32,
    sync_dir: String,
    configuration: EventConfig,
    count: u32,
    phantom: PhantomData<(S, I)>,
}

impl<S, I> core::fmt::Debug for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SyncOnDiskEventManager")
            .field("configuration", &self.configuration)
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

impl<S, I> Drop for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    fn drop(&mut self) {
    }
}

impl<S, I> SyncOnDiskEventManager<S, I>
where
    S: State + HasExecutions + HasCorpus,
    I: Input,
{
    /// Create a manager from a raw TCP client
    pub fn new(client_id: u32, sync_dir: String, configuration: EventConfig) -> Result<Self, Error> {
        Ok(Self{client_id: client_id, 
            sync_dir: sync_dir,
            configuration: configuration, 
            count: 0,
            phantom: PhantomData,
        })
    }

    pub fn existing_from_env(
        env_name: &str,
        sync_dir: String,
        configuration: EventConfig,
    ) -> Result<Self, Error> {
        let this_id = str::parse::<u32>(&env::var(env_name)?).unwrap();
        Ok(Self{client_id: this_id, 
            sync_dir: sync_dir,
            configuration: configuration, 
            count: 0,
            phantom: PhantomData,
        })
    }

    /// Write the client id for a client [`EventManager`] to env vars
    pub fn to_env(&self, env_name: &str) {
        env::set_var(env_name, format!("{}", self.client_id));
    }
}

impl<S, I> SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    pub fn send_exiting(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

impl<S, I> UsesState for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    type State = S;
}

impl<S, I> EventFirer for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    #[cfg(feature = "serialize_bytes")]
    fn fire(
        &mut self,
        _state: &mut Self::State,
        event: Event<<Self::State as UsesInput>::Input>,
        // event: &Event<I>,
    ) -> Result<(), Error> {
        
        match &event {
            Event::NewTestcase {
                input: _,
                client_config: _,
                exit_kind: _,
                corpus_size,
                observers_buf: _,
                time,
                executions,
                forward_id,
            } => {
                let serialized_event = bincode::serialize(&event).expect("Failed to serialize event");

                // Write the serialized binary data to a file
                let event_filename = format!("{}/client_{}_{}.event.presifuzz_lock", self.sync_dir, &self.client_id, self.count);
                
                #[cfg(feature = "debug")]
                println!("sync on dir: {}", self.sync_dir);
                
                let mut file = File::create(event_filename).expect("Failed to persist event on systemfile");
                file.write_all(&serialized_event).expect("Failed to write to file");
                    
                let event_filename = format!("{}/client_{}_{}.event.presifuzz_lock", self.sync_dir, &self.client_id, self.count);
                let new_filename = format!("{}/client_{}_{}.event", self.sync_dir, &self.client_id, self.count);
                fs::rename(event_filename, new_filename)?;

                self.count += 1;
            },
            Event::UpdateUserStats {
                name,
                value,
                phantom:_,
            } => {
                // println!("UpdateUserStats: {} -> {:?}", name, value);
                let serialized_event = serde_json::to_string(&event).expect("Failed to serialize event");

                //Write the serialized binary data to a file
                let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);

                #[cfg(feature = "debug")]
                println!("sync on dir: {}", self.sync_dir);
    
                if !Path::new(&new_filename).is_file() {
                    let mut file = File::create(event_filename).expect("Failed to persist event on systemfile");
                    file.write_all(serialized_event.as_bytes()).expect("Failed to write to file");

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(event_filename, new_filename)?;
                } else {
                    
                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(new_filename, event_filename)?;

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let mut file = OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(event_filename)
                        .unwrap();
                   
                    file.write_all("\n".as_bytes()).expect("Failed to write to file");
                    file.write_all(serialized_event.as_bytes()).expect("Failed to write to file");

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(event_filename, new_filename)?;
                }
            },
            _ => {}
        };

        Ok(())
    }

    #[cfg(not(feature = "serialize_bytes"))]
    fn fire(
        &mut self,
        _state: &mut Self::State,
        event: Event<<Self::State as UsesInput>::Input>,
        // event: &Event<I>,
    ) -> Result<(), Error> {
        
        match event {
            Event::NewTestcase {
                input: _,
                client_config: _,
                exit_kind: _,
                corpus_size: _,
                observers_buf: _,
                time: _,
                executions: _,
                forward_id: _,
            } => {
                let serialized_event = serde_json::to_string(&event).expect("Failed to serialize event");

                // Write the serialized binary data to a file
                let event_filename = format!("{}/client_{}_{}.event.presifuzz_lock", self.sync_dir, &self.client_id, self.count);

                #[cfg(feature = "debug")]
                println!("sync on dir: {}", self.sync_dir);

                let mut file = File::create(event_filename).expect("Failed to persist event on systemfile");
                file.write_all(serialized_event.as_bytes()).expect("Failed to write to file");
         
                let event_filename = format!("{}/client_{}_{}.event.presifuzz_lock", self.sync_dir, &self.client_id, self.count);
                let new_filename = format!("{}/client_{}_{}.event", self.sync_dir, &self.client_id, self.count);
                fs::rename(event_filename, new_filename)?;

                self.count += 1;
            },
            Event::UpdateUserStats {
                name:_,
                value:_,
                phantom:_,
            } => {
                // println!("UpdateUserStats: {} -> {:?}", name, value);
                let serialized_event = serde_json::to_string(&event).expect("Failed to serialize event");

                //Write the serialized binary data to a file
                let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);

                #[cfg(feature = "debug")]
                println!("sync on dir: {}", self.sync_dir);
    
                if !Path::new(&new_filename).is_file() {
                    let mut file = File::create(event_filename).expect("Failed to persist event on systemfile");
                    file.write_all(serialized_event.as_bytes()).expect("Failed to write to file");

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(event_filename, new_filename)?;
                } else {
                    
                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(new_filename, event_filename)?;

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let mut file = OpenOptions::new()
                        .write(true)
                        .append(true)
                        .open(event_filename)
                        .unwrap();
                   
                    file.write_all("\n".as_bytes()).expect("Failed to write to file");
                    file.write_all(serialized_event.as_bytes()).expect("Failed to write to file");

                    let event_filename = format!("{}/client_{}.stats_event.presifuzz_lock", self.sync_dir, &self.client_id);
                    let new_filename = format!("{}/client_{}.event.stats", self.sync_dir, &self.client_id);
                    fs::rename(event_filename, new_filename)?;
                }
            },
            _ => {}
        };
        Ok(())
    }

    fn configuration(&self) -> EventConfig {
        self.configuration
    }
}

impl<S, I> EventRestarter for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    fn await_restart_safe(&mut self) {
    }
}

impl<E, S, Z, I> EventProcessor<E, Z> for SyncOnDiskEventManager<S, I>
where
    S: State + HasExecutions + HasCorpus,
    E: HasObservers<State = S> + Executor<Self, Z>,
    for<'a> E::Observers: Deserialize<'a>,
    Z: EvaluatorObservers<E::Observers, State = S> + ExecutionProcessor<E::Observers, State = S>,
    I: Input,
{
    fn process(
        &mut self,
        _fuzzer: &mut Z,
        _state: &mut Self::State,
        _executor: &mut E,
    ) -> Result<usize, Error> {
        Ok(0)
    }
}

impl<E, S, Z, I> EventManager<E, Z> for SyncOnDiskEventManager<S, I>
where
    E: HasObservers<State = S> + Executor<Self, Z>,
    for<'a> E::Observers: Deserialize<'a>,
    S: State + HasExecutions + HasMetadata + HasLastReportTime + HasCorpus,
    Z: EvaluatorObservers<E::Observers, State = S> + ExecutionProcessor<E::Observers, State = S>,
    I: Input,
{
}

impl<S, I> ProgressReporter for SyncOnDiskEventManager<S, I> where
    S: State + HasExecutions + HasMetadata + HasLastReportTime+ HasCorpus,
    I: Input,
{
}

impl<S, I> HasEventManagerId for SyncOnDiskEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    /// Gets the id assigned to this staterestorer.
    fn mgr_id(&self) -> EventManagerId {
        EventManagerId(self.client_id as usize)
    }
}

/// A manager that can restart on the fly, storing states in-between (in `on_restart`)
#[cfg(feature = "std")]
#[derive(Debug)]
pub struct SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    mgr: SyncOnDiskEventManager<S, I>,
}

#[cfg(feature = "std")]
impl<S, I> UsesState for SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    type State = S;
}

#[cfg(feature = "std")]
impl<S, I> ProgressReporter for SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasExecutions + HasMetadata + HasLastReportTime + HasCorpus,
    I: Input,
{
}

#[cfg(feature = "std")]
impl<S, I> EventFirer for SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    fn fire(
        &mut self,
        state: &mut Self::State,
        event: Event<<Self::State as UsesInput>::Input>,
    ) -> Result<(), Error> {
        // Check if we are going to crash in the event, in which case we store our current state for the next runner
        self.mgr.fire(state, event)
    }

    fn serialize_observers<OT>(&mut self, observers: &OT) -> Result<Option<Vec<u8>>, Error>
    where
        OT: ObserversTuple<Self::State> + Serialize,
    {
        Ok(Some(postcard::to_allocvec(observers)?))
    }

    fn configuration(&self) -> EventConfig {
        self.mgr.configuration()
    }
}

#[cfg(feature = "std")]
impl<S, I> EventRestarter for SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasExecutions + HasCorpus,
    I: Input,
{
    /// The tcp client needs to wait until a broker mapped all pages, before shutting down.
    /// Otherwise, the OS may already have removed the shared maps,
    #[inline]
    fn await_restart_safe(&mut self) {
        self.mgr.await_restart_safe();
    }

    /// Reset the single page (we reuse it over and over from pos 0), then send the current state to the next runner.
    fn on_restart(&mut self, _state: &mut S) -> Result<(), Error> {
        self.await_restart_safe();
        Ok(())
    }

    fn send_exiting(&mut self) -> Result<(), Error> {
        // Also inform the broker that we are about to exit.
        // This way, the broker can clean up the pages, and eventually exit.
        self.mgr.send_exiting()
    }
}

#[cfg(feature = "std")]
impl<E, S, Z, I> EventProcessor<E, Z> for SyncOnDiskRestartingEventManager<S, I>
where
    E: HasObservers<State = S> + Executor<SyncOnDiskEventManager<S, I>, Z>,
    for<'a> E::Observers: Deserialize<'a>,
    S: State + HasExecutions + HasCorpus,
    Z: EvaluatorObservers<E::Observers, State = S> + ExecutionProcessor<E::Observers>, //CE: CustomEvent<I>,
    I: Input,
{
    fn process(&mut self, fuzzer: &mut Z, state: &mut S, executor: &mut E) -> Result<usize, Error> {
        self.mgr.process(fuzzer, state, executor)
    }
}

#[cfg(feature = "std")]
impl<E, S, Z, I> EventManager<E, Z> for SyncOnDiskRestartingEventManager<S, I>
where
    E: HasObservers<State = S> + Executor<SyncOnDiskEventManager<S, I>, Z>,
    for<'a> E::Observers: Deserialize<'a>,
    S: State + HasExecutions + HasMetadata + HasLastReportTime + HasCorpus,
    Z: EvaluatorObservers<E::Observers, State = S> + ExecutionProcessor<E::Observers>, //CE: CustomEvent<I>,
    I: Input,
{
}

#[cfg(feature = "std")]
impl<S, I> HasEventManagerId for SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    fn mgr_id(&self) -> EventManagerId {
        self.mgr.mgr_id()
    }
}

/// The tcp connection from the actual fuzzer to the process supervising it
const _ENV_FUZZER_SENDER: &str = "_AFL_ENV_FUZZER_SENDER";
const _ENV_FUZZER_RECEIVER: &str = "_AFL_ENV_FUZZER_RECEIVER";
/// The tcp (2 way) connection from a fuzzer to the broker (broadcasting all other fuzzer messages)
const _ENV_FUZZER_BROKER_CLIENT_INITIAL: &str = "_AFL_ENV_FUZZER_BROKER_CLIENT";

#[cfg(feature = "std")]
impl<S, I> SyncOnDiskRestartingEventManager<S, I>
where
    S: State + HasCorpus,
    I: Input,
{
    /// Create a new runner, the executed child doing the actual fuzzing.
    pub fn new(mgr: SyncOnDiskEventManager<S, I>) -> Self {
        Self {
            mgr,
        }
    }
}

/// The kind of manager we're creating right now
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy)]
pub enum SyncOnDiskManagerKind {
    /// Any kind will do
    // Any,
    /// A client, getting messages from a local broker.
    Client {
        /// The CPU core ID of this client
        cpu_core: Option<u32>,
    },
    /// A broker, forwarding all packets of local clients via TCP.
    Broker,
}

/// Sets up a restarting fuzzer, using the [`StdShMemProvider`], and standard features.
/// The restarting mgr is a combination of restarter and runner, that can be used on systems with and without `fork` support.
/// The restarter will spawn a new process each time the child crashes or timeouts.
#[cfg(feature = "std")]
#[allow(clippy::type_complexity)]
pub fn setup_restarting_mgr_sync_on_disk<MT, S, I>(
    monitor: MT,
    sync_dir: String,
    configuration: EventConfig,
) -> Result<(Option<S>, SyncOnDiskRestartingEventManager<S, I>), Error>
where
    MT: Monitor + Clone,
    S: State + HasExecutions + HasCorpus,
    I: Input,
{
    SyncOnDiskRestartingMgr::builder()
        .monitor(Some(monitor))
        .sync_dir(sync_dir)
        .configuration(configuration)
        .build()
        .launch()
}

/// Provides a `builder` which can be used to build a [`TcpRestartingMgr`], which is a combination of a
/// `restarter` and `runner`, that can be used on systems both with and without `fork` support. The
/// `restarter` will start a new process each time the child crashes or times out.
#[cfg(feature = "std")]
#[allow(clippy::default_trait_access, clippy::ignored_unit_patterns)]
#[derive(TypedBuilder, Debug)]
pub struct SyncOnDiskRestartingMgr<MT, S, I>
where
    S: UsesInput + DeserializeOwned,
    MT: Monitor,
    I: Input,
{
    /// The configuration
    configuration: EventConfig,
    /// The monitor to use
    #[builder(default = None)]
    monitor: Option<MT>,
    // The sync_dir where Event are saved
    sync_dir: String,
    /// The type of manager to build
    #[builder(default = SyncOnDiskManagerKind::Broker)]
    kind: SyncOnDiskManagerKind,
    #[builder(setter(skip), default = PhantomData)]
    phantom_data: PhantomData<(S, I)>,
}

#[cfg(feature = "std")]
#[allow(clippy::type_complexity, clippy::too_many_lines)]
impl<MT, S, I> SyncOnDiskRestartingMgr<MT, S, I>
where
    S: State + HasExecutions + HasCorpus,
    MT: Monitor + Clone,
    I: Input,
{

    /// Launch the restarting manager
    pub fn launch(&mut self) -> Result<(Option<S>, SyncOnDiskRestartingEventManager<S, I>), Error> {
            
        let broker_things = |mut broker: SyncOnDiskEventBroker<S::Input, MT>| {
            broker.broker_loop()
        };

        // Launch broker loop or start Client fuzzer
        match self.kind {
            SyncOnDiskManagerKind::Broker => {
                let event_broker = SyncOnDiskEventBroker::<S::Input, MT>::new(
                    self.sync_dir.clone(),
                    self.monitor.take().unwrap(),
                )?;

                broker_things(event_broker)?;
                unreachable!("The broker may never return normally, only on errors or when shutting down.");
            }
            SyncOnDiskManagerKind::Client { cpu_core } => {
                // We are a client
                let mgr = SyncOnDiskEventManager::<S, I>::new(cpu_core.unwrap(), self.sync_dir.clone(), self.configuration)?;
 
                let state = None;
                return Ok((state, SyncOnDiskRestartingEventManager::new(mgr)));
            }
        };
    }
}

