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
pub struct CSRLog {
  mstatus_xIE: u32,
  mstatus_xPIE: u32,
  mstatus_xPP: u32,
  mstatus_XS: u32,
  mstatus_FS: u32,
  mstatus_MPRV: u32,
  mstatus_SUM: u32,
  mstatus_MXR: u32,
  mstatus_TVM: u32,
  mstatus_TW: u32,
  mstatus_TSR: u32,
  mstatus_xXL: u32,
  mstatus_SD: u32,
  mscause: u32,
  medeleg: u32,
  mscounteren: u32,
  frm: u32,
  fflags: u32
}

// Implement PartialEq for CSRLog
impl PartialEq for CSRLog {
    fn eq(&self, other: &Self) -> bool {
        self.mstatus_xIE == other.mstatus_xIE &&
        self.mstatus_xPIE == other.mstatus_xPIE &&
        self.mstatus_xPP == other.mstatus_xPP &&
        self.mstatus_XS == other.mstatus_XS &&
        self.mstatus_FS == other.mstatus_FS &&
        self.mstatus_MPRV == other.mstatus_MPRV &&
        self.mstatus_SUM == other.mstatus_SUM &&
        self.mstatus_MXR == other.mstatus_MXR &&
        self.mstatus_TVM == other.mstatus_TVM &&
        self.mstatus_TW == other.mstatus_TW &&
        self.mstatus_TSR == other.mstatus_TSR &&
        self.mstatus_xXL == other.mstatus_xXL &&
        self.mstatus_SD == other.mstatus_SD &&
        self.mscause == other.mscause &&
        self.medeleg == other.medeleg &&
        self.mscounteren == other.mscounteren &&
        self.frm == other.frm &&
        self.fflags == other.fflags
    }
}

impl CSRLog {
    pub fn from_array(values: [u32; 18]) -> Self {
        CSRLog {
            mstatus_xIE: values[0],
            mstatus_xPIE: values[1],
            mstatus_xPP: values[2],
            mstatus_XS: values[3],
            mstatus_FS: values[4],
            mstatus_MPRV: values[5],
            mstatus_SUM: values[6],
            mstatus_MXR: values[7],
            mstatus_TVM: values[8],
            mstatus_TW: values[9],
            mstatus_TSR: values[10],
            mstatus_xXL: values[11],
            mstatus_SD: values[12],
            mscause: values[13],
            medeleg: values[14],
            mscounteren: values[15],
            frm: values[16],
            fflags: values[17],
        }
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum OpType {
    Read,
    Write,
}

// Implement PartialEq for OpType
impl PartialEq for OpType {
    fn eq(&self, other: &Self) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct MemOp {
    pub op_type: OpType,
    pub address: u64,
    pub value: u64,

}

// Implement PartialEq for MemOp
impl PartialEq for MemOp {
    fn eq(&self, other: &Self) -> bool {
        self.op_type == other.op_type &&
        self.address == other.address &&
        self.value == other.value
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RegOp {
    pub op_type: OpType,
    pub name: String,
    pub value: u64,
}

// Implement PartialEq for RegOp
impl PartialEq for RegOp {
    fn eq(&self, other: &Self) -> bool {
        self.op_type == other.op_type &&
        self.name == other.name &&
        self.value == other.value
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OpLog {
    RegOp(RegOp),
    MemOp(MemOp),
}

impl PartialEq for OpLog {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OpLog::RegOp(a), OpLog::RegOp(b)) => a == b,
            (OpLog::MemOp(a), OpLog::MemOp(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TraceLog {
    pub pc: u64,
    pub inst: u64,
    pub ops: Vec<OpLog>,
    pub csr: Option<CSRLog>,
}

pub trait ExecTraceParser {
    fn new() -> Self;
    fn parse(&self, trace_filename: &str) -> Result<Vec<TraceLog>, Error> ;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct ExecTraceObserver<T>
{
    name: String,
    trace_filename: String,
    trace: Vec<TraceLog>,
    trace_parser: T,
}

impl<T> ExecTraceObserver<T> 
where
    T: ExecTraceParser,
{
    pub fn new(name: &str, trace_filename: &str) -> Self {
        Self {
            name: name.to_string(),
            trace: Vec::<TraceLog>::new(),
            trace_filename: trace_filename.to_string(),
            trace_parser: T::new(),
        }
    }

    pub fn cnt(&self) -> usize {
        self.trace.len()
    }
    
    pub fn trace(&self) -> &Vec<TraceLog> {
        &self.trace
    }
}


impl<T> Named for ExecTraceObserver<T>
where
    T: ExecTraceParser,
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl<T> HasLen for ExecTraceObserver<T>
where
    T: ExecTraceParser,
{
    fn len(&self) -> usize {
        self.trace.len()
    }
}

impl<S,T> Observer<S> for ExecTraceObserver<T>
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
        
        self.trace = self.trace_parser.parse(&self.trace_filename).unwrap();

        Ok(())
    }
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct SpikeExecTraceObserver;

impl ExecTraceParser for SpikeExecTraceObserver 
{
    fn new() -> Self {
        SpikeExecTraceObserver {}
    }
    fn parse(&self, trace_filename: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();

        let spike_file = trace_filename;
        
        let file = File::open(spike_file).expect("Unable to open spike trace file");
        let reader = io::BufReader::new(file);

        let spike_store_commit_re = Regex::new(r"core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+mem\s+0x(\w+)\s+0x(\w+)").unwrap();
        let spike_rest_commit_re = Regex::new(r"core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+(\w+)\s+0x(\w+)(\s+(\w+)\s+0x(\w+))?").unwrap();
        for line in reader.lines() {
            if let Ok(log_line) = &line {
                if let Some(caps) = spike_store_commit_re.captures(log_line) {
                    let ops = vec![
                        OpLog::MemOp(MemOp{op_type: OpType::Write, address: u64::from_str_radix(&caps[3], 16).unwrap(), value: u64::from_str_radix(&caps[4], 16).unwrap()})
                    ];
                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        csr: None
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
                        csr: None
                    });
                }
            }
        }
        Ok(trace)
    }   
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct ProcessorFuzzExecTraceObserver;

impl ExecTraceParser for ProcessorFuzzExecTraceObserver 
{
    fn new() -> Self {
        ProcessorFuzzExecTraceObserver {}
    }
    fn parse(&self, trace_filename: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();

        let spike_file = trace_filename;
        
        let file = File::open(spike_file).expect("Unable to open spike trace file");
        let reader = io::BufReader::new(file);

        let spike_store_commit_re = Regex::new(r"core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+mem\s+0x(\w+)\s+0x(\w+)").unwrap();
        let spike_rest_commit_re = Regex::new(r"core\s+\d+: \d+ 0x(\w+) \(0x(\w+)\)\s+(\w+)\s+0x(\w+)(\s+(\w+)\s+0x(\w+))?").unwrap();
        for line in reader.lines() {

            let mut csr_values = [0u32; 18];

            if let Ok(log_line) = &line {

                if let Some(caps) = spike_store_commit_re.captures(log_line) {
    
                    for (i, cap) in caps.iter().skip(3).enumerate() {
                        if let Some(cap) = cap {
                            csr_values[i] = cap.as_str().parse().unwrap();
                        }
                    }
                    let ops = vec![
                        OpLog::MemOp(MemOp{op_type: OpType::Write, address: u64::from_str_radix(&caps[3], 16).unwrap(), value: u64::from_str_radix(&caps[4], 16).unwrap()})
                    ];
                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        csr: Some(CSRLog::from_array(csr_values))
                    });
                }
                else if let Some(caps) = spike_rest_commit_re.captures(log_line) {
                    for (i, cap) in caps.iter().skip(3).enumerate() {
                        if let Some(cap) = cap {
                            csr_values[i] = cap.as_str().parse().unwrap();
                        }
                    }
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
                        csr: Some(CSRLog::from_array(csr_values))
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
    use crate::trace_observer::{ExecTraceObserver, SpikeExecTraceObserver};
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

        let mut spike_trace_observer  = ExecTraceObserver::<SpikeExecTraceObserver>::new("spike_trace_observer", "./spike.err");

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
        println!("{:?}", spike_trace_observer.trace.len())
    }
}

