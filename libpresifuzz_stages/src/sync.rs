// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! The [`SyncFromDiskStage`] is a stage that imports inputs from disk for e.g. sync with AFL

use core::marker::PhantomData;
use std::{
    fs,
    path::{PathBuf},
    time::SystemTime,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "introspection")]
use crate::state::HasClientPerfMonitor;
use libafl::{
    corpus::{CorpusId, HasTestcase},
    events::{Event, EventFirer},
    executors::{Executor, HasObservers},
    fuzzer::{EvaluatorObservers, ExecutionProcessor},
    inputs::{Input, UsesInput},
    stages::Stage,
    state::{HasCorpus, HasExecutions, HasMetadata, HasRand, State, UsesState},
    Error,
};

use std::fs::File;
use std::io::{Read};

use libpresifuzz_feedbacks::transferred::TransferringMetadata;

/// Metadata used to store information about disk sync time
#[cfg_attr(
    any(not(feature = "serdeany_autoreg"), miri),
    allow(clippy::unsafe_derive_deserialize)
)] // for SerdeAny
#[derive(Serialize, Deserialize, Debug)]
pub struct SyncFromDiskMetadata {
    /// The last time the sync was done
    pub last_time: SystemTime,
}

libafl_bolts::impl_serdeany!(SyncFromDiskMetadata);

impl SyncFromDiskMetadata {
    /// Create a new [`struct@SyncFromDiskMetadata`]
    #[must_use]
    pub fn new(last_time: SystemTime) -> Self {
        Self { last_time: last_time}
    }
}

/// A stage that loads testcases from disk to sync with other fuzzers such as AFL++
#[derive(Debug)]
pub struct SyncFromDiskStage<S, DI>
where
    S: State,
    DI: Input,
{
    sync_dir: PathBuf,
    loaded_testcases: u32,
    phantom: PhantomData<(S, DI)>,
}

impl<S, DI> UsesState for SyncFromDiskStage<S, DI>
where
    S: State,
    DI: Input,
{
    type State = S;
}

impl<DI, E, EM, S, Z> Stage<E, EM, Z> for SyncFromDiskStage<S, DI>
where
    EM: UsesState<State = S> + EventFirer,
    S: State + HasExecutions + HasCorpus + HasRand + HasMetadata + HasTestcase + UsesInput<Input = DI>,
    E: HasObservers<State = S> + Executor<EM, Z>,
    for<'a> E::Observers: Deserialize<'a>,
    Z: EvaluatorObservers<E::Observers, State = S> + ExecutionProcessor<E::Observers, State = S>,
    DI: Input,
{
    #[inline]
    fn perform(
        &mut self,
        fuzzer: &mut Z,
        _executor: &mut E,
        state: &mut Z::State,
        manager: &mut EM,
        _corpus_idx: CorpusId,
    ) -> Result<(), Error> {
        let last = state
            .metadata_map()
            .get::<SyncFromDiskMetadata>()
            .map(|m| m.last_time);

        let in_dir = self.sync_dir.clone();
        if let Some(mut max_time) = {
            let mut max_time = None;

            let it = fs::read_dir(in_dir);
            if it.is_err() {
                println!("enable to read sync_dir {:?}. Maybe it does not exist?", self.sync_dir);
                return Ok(());
            }

            for entry in it.unwrap() {
                let entry = entry.expect("Unable to read files in sync_dir");
                let path = entry.path();
                let attributes = fs::metadata(&path);

                if attributes.is_err() {
                    continue;
                }

                let attr = attributes.expect("Unable to read files attributes in sync_dir");

                if attr.is_file() && attr.len() > 0 {

                    if path.file_name().is_some() {
                        
                        let filename = path.file_name().unwrap();

                        if filename.to_str().is_some() {
                            let filename = filename.to_str().unwrap();

                            if filename.contains(".stats") {
                                continue;
                            }
                        }
                    }

                    if let Ok(time) = attr.modified() {
                            
                        // Read the file back and deserialize the Event struct
                        let path_str = path.to_str().expect("Failed to convert path to string");
                        let mut file = File::open(path_str).expect("Failed to open file");

                        if let Some(l) = last {
                            let elapsed = time.duration_since(l);

                            if elapsed.is_err() {
                                continue;
                            }
                      
                            let elapsed = elapsed.unwrap(); 
                            if elapsed.as_nanos() == 0 {
                                continue;
                            } 

                            println!("+ testcase: {}; total: {}; elapsed: {}s {}ms {}ns", path_str, self.loaded_testcases, elapsed.as_secs(), elapsed.as_millis(), elapsed.as_nanos());
                        
                            max_time = Some(max_time.map_or(time, |t: SystemTime| t.max(time)));
                       } else {
                            
                            println!("+ reloading testcase: {}; total: {};", path_str, self.loaded_testcases);
                       }
                                    
                        self.loaded_testcases += 1; 

                        // Read the file back and deserialize the Event struct
                        #[cfg(not(feature = "serialize_bytes"))]
                        {
                            let mut serialized_event = String::new();
                            file.read_to_string(&mut serialized_event).expect("Failed to read file");

                            // Deserialize the JSON string back into an Event struct
                            let event = serde_json::from_str(&serialized_event);
                            if !event.is_ok() {
                                println!("Skipping testcase loading as it might be corrupted");
                                continue; 
                            }
                            let event: Event<DI> = event.unwrap();
                            
                            match &event {
                                Event::NewTestcase {
                                    // input: DI,
                                    input,
                                    client_config: _,
                                    exit_kind,
                                    corpus_size: _,
                                    observers_buf,
                                    time: _,
                                    executions: _,
                                    forward_id: _,
                                } => {

                                    let observers: E::Observers =
                                        postcard::from_bytes(observers_buf.as_ref().unwrap()).expect("Unable to deserialize new testcase from sync_dir");

                                    if let Ok(meta) = state.metadata_mut::<TransferringMetadata>() {
                                        meta.set_transferring(true);
                                    }

                                    fuzzer.process_execution(state, manager, input.clone(), &observers, &exit_kind, false)?;

                                    if let Ok(meta) = state.metadata_mut::<TransferringMetadata>() {
                                        meta.set_transferring(false);
                                    }
                                }
                                _ => {},
                            };
                        }
                        
                        #[cfg(feature = "serialize_bytes")]
                        {
                            let mut serialized_event = Vec::new();
                            file.read_to_end(&mut serialized_event).expect("Failed to read file");

                            // Deserialize the binary data back into an Event struct
                            let event: Event<DI> = bincode::deserialize(&serialized_event).expect("Failed to deserialize event");
                            match &event {
                                Event::NewTestcase {
                                    // input: DI,
                                    input,
                                    client_config: _,
                                    exit_kind,
                                    corpus_size,
                                    observers_buf,
                                    time,
                                    executions,
                                    forward_id,
                                } => {
                                   let observers: E::Observers =
                                        postcard::from_bytes(observers_buf.as_ref().unwrap()).expect("Unable to deserialize new testcase from sync_dir");
                                    
                                   println!("+ testcase: {}; total: {}", path_str, self.loaded_testcases);

                                    self.loaded_testcases += 1; 

                                    fuzzer.process_execution(state, manager, input.clone(), &observers, &exit_kind, false)?;
                                }
                                _ => return Err(Error::unknown(format!(
                                    "Received illegal message that message should not have arrived: {:?}.",
                                    event.name()
                                ))),
                            };
                        }
                    }
                } 
            }

            Some(max_time)
        }

        {

            if max_time.is_none() {
                max_time = Some(SystemTime::now());
            }

            if last.is_none() {
                state
                    .metadata_map_mut()
                    .insert(SyncFromDiskMetadata::new(max_time.unwrap()));
            } else {
                state
                    .metadata_map_mut()
                    .get_mut::<SyncFromDiskMetadata>()
                    .unwrap()
                    .last_time = max_time.unwrap();
            }
        }

        #[cfg(feature = "introspection")]
        state.introspection_monitor_mut().finish_stage();

        Ok(())
    }

}

impl<S, DI> SyncFromDiskStage<S, DI>
where
    S: State,
    DI: Input
{
    /// Creates a new [`SyncFromDiskStage`]
    #[must_use]
    pub fn new(sync_dir: PathBuf) -> Self {
        Self {
            sync_dir: sync_dir,
            loaded_testcases: 0,
            phantom: PhantomData,
        }
    }
}
