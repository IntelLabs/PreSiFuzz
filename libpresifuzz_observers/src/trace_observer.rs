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
use std::str::FromStr;
use std::num::ParseIntError;

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
    Unknown,
    Trap
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
            OpType::Trap => write!(f, "Trap"),
            OpType::Unknown => write!(f, "Unknown")
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
pub struct TrapOp {
    time_ns: u64,
    exec_address: u64,
    cause: String,
    tval: u64,
}


// Implement PartialEq for RegOp
impl PartialEq for TrapOp {
    fn eq(&self, other: &Self) -> bool {
        self.exec_address == other.exec_address &&
        self.cause == other.cause &&
        self.tval == other.tval
    }
}

impl Display for TrapOp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TrapOp {{ exec_address: {}, cause: {}, tval: {}, time: {}ns }}",
            self.exec_address, self.cause, self.tval, self.time_ns
        )
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum OpLog {
    RegOp(RegOp),
    MemOp(MemOp),
    TrapOp(TrapOp),
}

impl PartialEq for OpLog {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (OpLog::RegOp(a), OpLog::RegOp(b)) => a == b,
            (OpLog::MemOp(a), OpLog::MemOp(b)) => a == b,
            (OpLog::TrapOp(a), OpLog::TrapOp(b)) => a == b,
            _ => false,
        }
    }
}

impl Display for OpLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OpLog::RegOp(reg_op) => write!(f, "RegOp({})", reg_op),
            OpLog::MemOp(mem_op) => write!(f, "MemOp({})", mem_op),
            OpLog::TrapOp(trap_op) => write!(f, "TrapOp({})", trap_op),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TraceLog {
    pub pc: u64,
    pub inst: u64,
    pub ops: Vec<OpLog>,
    pub csr: Option<CSRLog>,

    // optional information
    pub time_ns: u64,
    pub cycles: u32,
    pub exec_mode: char,
    pub disassembly: String,
}

impl TraceLog {
    pub fn new(pc: u64, inst: u64, ops: Vec<OpLog>, csr: Option<CSRLog>) -> Self {
        Self{
            pc:pc,
            inst:inst,
            ops:ops,
            csr:csr,
            time_ns:0,
            cycles:0,
            exec_mode:'-',
            disassembly:"".to_string()
        }
    }
}

impl Display for TraceLog {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "TraceLog {{ pc: {}, inst: {}, ops: {:?}, csr: {:?} }} optional info {{ time: {}ns, cycles: {}, exec mode: {}, disassembly: {} }}",
            self.pc,
            self.inst,
            self.ops.iter().map(|op| format!("{}", op)).collect::<Vec<_>>().join(", "),
            self.csr,
            self.time_ns,
            self.cycles,
            self.exec_mode,
            self.disassembly
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
                    trace.push(TraceLog::new(
                        u64::from_str_radix(&caps[1], 16).unwrap(),
                        u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        None
                    ));
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
                    
                    trace.push(TraceLog::new(
                        u64::from_str_radix(&caps[1], 16).unwrap(),
                        u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        None
                    ));
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
        let mut csr = None;

        for line in reader.lines() {
            if let Ok(log_line) = &line {

                let re = Regex::new(r"\[(.*?)\]").unwrap();

                if let Some(caps) = re.captures(log_line) {
                        let bracket_content = &caps[1];

                        let mut numbers = [0u32; 9];
                        let parsed_numbers: Vec<u32> = bracket_content
                            .split(',')
                            .filter_map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).ok())
                            .collect();

                        if parsed_numbers.len() == 9 {
                            numbers.copy_from_slice(&parsed_numbers);
                        } else {
                            panic!("Error: Processorfuzz log format not enforced by Spike");
                        }
                
                        csr = Some(CSRLog::from_array(numbers));
                } 

                if let Some(caps) = spike_store_commit_re.captures(log_line) {
                    let ops = vec![
                        OpLog::MemOp(MemOp{op_type: OpType::Write, address: u64::from_str_radix(&caps[3], 16).unwrap(), value: u64::from_str_radix(&caps[4], 16).unwrap()})
                    ];
                    trace.push(TraceLog::new(
                        u64::from_str_radix(&caps[1], 16).unwrap(),
                        u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        csr
                    ));
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
                    
                    trace.push(TraceLog::new(
                        u64::from_str_radix(&caps[1], 16).unwrap(),
                        u64::from_str_radix(&caps[2], 16).unwrap(),
                        ops,
                        csr
                    ));
                }
            }
        }
        Ok(trace)
    }   
}

// fn main() {
    // let text = r#"
    // Exception @     18870, PC: 800023e0, Cause: Breakpoint,
                                    // tval: 0000000000009002
       // 19010ns      943 M 80002018 0 341022f3 csrr           t0, mepc              t0  :00000000800023e0
       // 19110ns      948 M 8000201c 0 00028303 lb             t1, 0(t0)             t1  :0000000000000002 t0  :00000000800023e0 VA: 800023e0 PA: 0800023e0
       // 19130ns      949 M 80002020 0 0000450d c.li           a0, 3                 a0  :0000000000000003
       // 19150ns      950 M 80002022 0 00a37333 and            t1, t1, a0            t1  :0000000000000002 t1  :0000000000000002 a0  :0000000000000003
       // 19170ns      951 M 80002026 0 90000eb7 lui            t4, 0x90000           t4  :0000000090000000
       // 19230ns      954 M 8000202a 0 000eaf03 lw             t5, 0(t4)             t5  :0000000000000009 t4  :0000000090000000 VA: 90000000 PA: 090000000
       // 19270ns      956 M 8000202e 0 00000f05 c.addi         t5, t5, 1             t5  :000000000000000a t5  :0000000000000009
       // 19290ns      957 M 80002030 0 00004fa9 c.li           t6, 10                t6  :000000000000000a
       // 19310ns      958 M 80002032 1 01ff0f63 beq            t5, t6, pc + 30       t5  :000000000000000a t6  :000000000000000a
       // 19430ns      964 M 80002050 0 00000e17 auipc          t3, 0x0               t3  :0000000080002050
       // 19470ns      966 M 80002054 0 010e0e13 addi           t3, t3, 16            t3  :0000000080002060 t3  :0000000080002050
       // 19490ns      967 M 80002058 0 141e1073 csrw           t3, sepc              t3  :0000000080002060
       // 19530ns      969 M 8000205c 0 30200073 mret
    // "#;
//
    // let entries = parse_log(text);
    // for entry in entries {
        // println!("{:?}", entry);
    // }
// }

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct CVA6ExecTrace;

impl ExecTraceParser for CVA6ExecTrace 
{
    fn new() -> Self {
        CVA6ExecTrace {}
    }

    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();

        //TODO: pass trace file name in contructor
        let cva6_trace_file = format!("{}/trace_hart_0.log", workdir);
        let file = File::open(cva6_trace_file).expect("Unable to open cva6 trace file");
        let reader = io::BufReader::new(file);

        for line in reader.lines() {
            
            let line = line.unwrap();

            if line.trim().starts_with("Exception") {
                if let Some(entry) = self.parse_exception_line(&line) {
                    trace.push(entry);
                }
            } else if let Some(entry) = self.parse_record_line(&line) {
                trace.push(entry);
            }

        }

        Ok(trace)
    }
    // OpLog::RegOp(RegOp{op_type: OpType::Read, name: caps[6].to_string(), value: u64::from_str_radix(&caps[7], 16).unwrap()})
    // OpLog::RegOp(RegOp{op_type: OpType::Write, name: caps[2].to_string(), value: u64::from_str_radix(&caps[3], 16).unwrap()}),
}


impl CVA6ExecTrace 
{

    fn parse_hex_u64(&self, s: &str) -> Result<u64, ParseIntError> {
        u64::from_str_radix(s, 16)
    }

    fn parse_record_line(&self, line: &str) -> Option<TraceLog> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 7 {
            return None;
        }

        let time_str = parts[0];
        let time_ns = time_str.trim_end_matches("ns").parse().ok()?;

        let cycles = parts[1].parse().ok()?;
        let exec_mode = parts[2].chars().next()?;
        let pc = self.parse_hex_u64(parts[3]).ok()?;
        let inst = self.parse_hex_u64(parts[5]).ok()?;

        // Parsing disassembly and register fields
        let mut disassembly_end = 6; // Disassembly starts at index 6
        let mut disassembly = String::new();

        // Extract the mnemonic and operands until the first unseparated word
        if disassembly_end < parts.len() {
            disassembly.push_str(parts[disassembly_end]);
            disassembly_end += 1;
        }

        while disassembly_end < parts.len() {
            if parts[disassembly_end].ends_with(',') {
                disassembly.push(' ');
                disassembly.push_str(parts[disassembly_end]);
            } else {
                disassembly.push(' ');
                disassembly.push_str(parts[disassembly_end]);
                disassembly_end += 1;
                break;
            }
            disassembly_end += 1;
        }
                    
        let mut ops = vec![];

        // Parsing register assignments
        for register in parts[disassembly_end..].chunks(2) {
            println!("{:?}", register);
            
            if register.len() == 2 {
                let reg_name = register[1].trim_start_matches(':').to_string();
                if let Ok(reg_value) = self.parse_hex_u64(register[1]) {

                    let op = OpLog::RegOp(RegOp{op_type: OpType::Unknown, name: reg_name, value: reg_value});
                    ops.push(op); 
                }
            }
        }

        //TODO
        // Check if csr affected
        let csr = None;

        Some(TraceLog{
            pc,
            inst,
            ops,
            csr,
            
            time_ns,
            cycles,
            exec_mode,
            disassembly,
        })
    }

    fn parse_exception_line(&self, line: &str) -> Option<TraceLog> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 8 {
            return None;
        }

        // let time_ns = parts[2].trim_end_matches(',').parse().ok()?;
        let exec_address = self.parse_hex_u64(parts[4].trim_end_matches(',')).ok()?;
        let cause = parts[6].to_string();
        let tval = self.parse_hex_u64(parts[8]).ok()?;
        let mut ops = vec![
            OpLog::RegOp(RegOp{op_type: OpType::Trap, name: cause, value: tval})
        ];

        Some(TraceLog::new(
            exec_address,
            0,
            ops,
            None,
        ))
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

