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
use std::fmt::{self, Display, Formatter};

use std::fs::File;

extern crate fs_extra;
use std::{str};
use regex::Regex;
use std::io::{self, BufRead};

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct CSRLog {
  //mstatus_xIE: u32,
  //mstatus_xPIE: u32,
  //mstatus_xPP: u32,
  //mstatus_XS: u32,
  //mstatus_FS: u32,
  //mstatus_MPRV: u32,
  //mstatus_SUM: u32,
  //mstatus_MXR: u32,
  //mstatus_TVM: u32,
  //mstatus_TW: u32,
  //mstatus_TSR: u32,
  //mstatus_xXL: u32,
  //mstatus_SD: u32,
  pub mstatus: u32,
  pub frm: u32,
  pub fflags: u32,
  pub mcause: u32,
  pub scause: u32,
  pub medeleg: u32,
  pub mcounteren: u32,
  pub scounteren: u32,
  pub dcsr: u32,
}

// Implement PartialEq for CSRLog
impl PartialEq for CSRLog {
    fn eq(&self, other: &Self) -> bool {
        self.mstatus == other.mstatus &&
        self.frm == other.frm &&
        self.fflags == other.fflags &&
        self.mcause == other.mcause &&
        self.scause == other.scause &&
        self.medeleg == other.medeleg &&
        self.mcounteren == other.mcounteren &&
        self.scounteren == other.scounteren &&
        self.dcsr == other.dcsr
    }
}

impl CSRLog {
    pub fn from_array(values: [u32; 9]) -> Self {
        CSRLog {
            mstatus: values[0],
            frm: values[1],
            fflags: values[2],
            mcause: values[3],
            scause: values[4],
            medeleg: values[5],
            mcounteren: values[6],
            scounteren: values[7],
            dcsr: values[8],
        }
    }
}

impl Display for CSRLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CSRLog {{ mstatus: {}, frm: {}, fflags: {}, mcause: {}, scause: {}, medeleg: {}, mcounteren: {}, scounteren: {} }}",
            self.mstatus, self.frm, self.fflags, self.mcause, self.scause, self.medeleg, self.mcounteren, self.scounteren
        )
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

impl Display for OpType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OpType::Read => write!(f, "Read"),
            OpType::Write => write!(f, "Write"),
        }
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

impl Display for MemOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MemOp {{ op_type: {}, address: {}, value: {} }}",
            self.op_type, self.address, self.value
        )
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

impl Display for RegOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RegOp {{ op_type: {}, name: {}, value: {} }}",
            self.op_type, self.name, self.value
        )
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

impl Display for OpLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OpLog::RegOp(reg_op) => write!(f, "RegOp({})", reg_op),
            OpLog::MemOp(mem_op) => write!(f, "MemOp({})", mem_op),
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

impl Display for TraceLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TraceLog {{ pc: {}, inst: {}, ops: {:?}, csr: {} }}",
            self.pc,
            self.inst,
            self.ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join(", "),
            match &self.csr {
                Some(csr_log) => format!("{}", csr_log),
                None => "None".to_string(),
            }
        )
    }
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
            if let Ok(log_line) = &line {

                let re = Regex::new(r"\[(.*?)\]").unwrap();
                let csr = if let Some(caps) = re.captures(log_line) {
                        let bracket_content = &caps[1];

                        let mut numbers = [0u32; 9];
                        let parsed_numbers: Vec<u32> = bracket_content
                            .split(',')
                            .filter_map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                            .collect();

                        if parsed_numbers.len() == 9 {
                            numbers.copy_from_slice(&parsed_numbers);
                        } else {
                            println!("{:?}", parsed_numbers);
                            panic!("Error: Processorfuzz log format not enforced by Spike");
                        }

                        Some(CSRLog::from_array(numbers))
                } else {None};

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

