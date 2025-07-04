// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::{
    executors::{ExitKind},
    observers::{Observer},
    Error,
    inputs::{UsesInput},
};

use core::{fmt::Debug};
use serde::{Deserialize, Serialize};
use libafl_bolts::{HasLen, Named};

use std::fs::File;

extern crate fs_extra;
use std::{str};
use regex::Regex;
use std::io::{self, BufRead};




#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum OpType {
    Read,
    Write,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct MemOp {
    pub op_type: OpType,
    pub address: u64,
    pub value: u64,

}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RegOp {
    pub op_type: OpType,
    pub name: String,
    pub value: u64,

}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OpLog {
    RegOp(RegOp),
    MemOp(MemOp),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TraceLog {
    pub pc: u64,
    pub inst: u64,
    pub ops: Vec<OpLog>,
}

pub trait ExecTraceParser {
    fn new() -> Self;
    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> ;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct ExecTrace<T>
{
    name: String,
    workdir: String,
    trace: Vec<TraceLog>,
    trace_parser: T,
}

impl<T> ExecTrace<T> 
where
    T: ExecTraceParser,
{
    pub fn new(name: &str, workdir: &str) -> Self {
        Self {
            name: name.to_string(),
            trace: Vec::<TraceLog>::new(),
            workdir: workdir.to_string(),
            trace_parser: T::new(),
        }
    }

    pub fn cnt(&self) -> usize {
        self.trace.len()
    }
}


impl<T> Named for ExecTrace<T>
where
    T: ExecTraceParser,
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl<T> HasLen for ExecTrace<T>
where
    T: ExecTraceParser,
{
    fn len(&self) -> usize {
        self.trace.len()
    }
}

impl<S,T> Observer<S> for ExecTrace<T>
where
    S: UsesInput,
    T: ExecTraceParser,
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn post_exec(
       &mut self,
        _state: &mut S,
        _input: &S::Input,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {
        
        self.trace = self.trace_parser.parse(&self.workdir).unwrap();

        Ok(())
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct SpikeExecTrace;

impl ExecTraceParser for SpikeExecTrace 
{
    fn new() -> Self {
        SpikeExecTrace {}
    }
    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();

        let spike_file = format!("{}/spike.err", workdir);
        
        let file = File::open(spike_file).expect("Unable to open spike trace file");
        let reader = io::BufReader::new(file);
        let spike_store_commit_re = Regex::new(r"^core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+mem\s+0x(\w+)\s+0x(\w+)$").unwrap();
        let spike_jmps_re = Regex::new(r"^core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)$").unwrap();
        let spike_rest_commit_re = Regex::new(r"^core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+(\w+)\s+0x(\w+)(\s+(\w+)\s+0x(\w+))?$").unwrap();
        for line in reader.lines() {
            if let Ok(log_line) = &line {
                if let Some(caps) = spike_store_commit_re.captures(log_line) {
                    let ops: Vec<OpLog> = vec![
                        OpLog::MemOp(MemOp{op_type: OpType::Write, address: u64::from_str_radix(&caps[3], 16).unwrap(), value: u64::from_str_radix(&caps[4], 16).unwrap()})
                    ];
                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                    });
                }
                else if let Some(caps) = spike_jmps_re.captures(log_line) {
                    let ops: Vec<OpLog> = vec![];
                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                    });
                }
                else if let Some(caps) = spike_rest_commit_re.captures(log_line) {
                    let mut ops = vec![
                        OpLog::RegOp(RegOp{op_type: OpType::Read, name: caps[3].to_string(), value: u64::from_str_radix(&caps[4], 16).unwrap()})
                    ];
            
                    if caps.get(5) != None && &caps[6] == "mem" {
                        ops.push(OpLog::MemOp(MemOp{op_type: OpType::Read, address: u64::from_str_radix(&caps[7], 16).unwrap(), value: u64::from_str_radix(&caps[4], 16).unwrap()}));
                    } else if caps.get(5) != None {
                        ops.push(OpLog::RegOp(RegOp{op_type: OpType::Read, name: caps[6].to_string(), value: u64::from_str_radix(&caps[7], 16).unwrap()}));
                    }
                    
                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                    });
                }
            }
        }
        Ok(trace)
    }   
}



// TODO: Re-enable this test using vdb from open source design
#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    extern crate fs_extra;
    use libafl_bolts::prelude::StdRand;
    use libafl::prelude::BytesInput;
    use libafl::executors::{ExitKind};
    use libafl_bolts::current_time;
    use libafl::prelude::InMemoryCorpus;
    use libafl::prelude::ConstFeedback;
    use crate::trace_observer::{ExecTrace, SpikeExecTrace};
    use libafl::prelude::StdState;
    use libafl::state::HasMaxSize;
    use libafl::observers::Observer;

    #[test]
    fn test_spike_trace_observer() {

        let input = BytesInput::new(vec![1, 2, 3, 4]);

        let rand = StdRand::with_seed(current_time().as_nanos() as u64);
        let corpus = InMemoryCorpus::<BytesInput>::new();

        let mut feedback = ConstFeedback::new(true);
        let mut objective = ConstFeedback::new(false);

        let mut spike_trace_observer  = ExecTrace::<SpikeExecTrace>::new("spike_trace", "./");

        let mut state = StdState::new(
            rand,
            corpus,
            InMemoryCorpus::<BytesInput>::new(),
            &mut feedback,
            &mut objective,
        )
        .unwrap();
        state.set_max_size(1024);

        let _ = spike_trace_observer.post_exec(&mut state, &input, &ExitKind::Ok);
        //TODO: For now we do not consider exceptions, so only 147
        assert!(spike_trace_observer.trace.len() == 147);
    }
}

