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

use libpresifuzz_observers::trace_observer::{ExecTraceParser, TraceLog, OpLog, RegOp, OpType};


#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub struct RocketExecTrace;

impl ExecTraceParser for RocketExecTrace 
{
    fn new() -> Self {
        RocketExecTrace {}
    }
    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();

        let rocket_trace_file = format!("{}/terminal.err", workdir);

        let file = File::open(rocket_trace_file).expect("Unable to open rocket trace file");
        let reader = io::BufReader::new(file);

        let rocket_re = Regex::new(r"C0:\s+\d+ \[\d+\] pc=\[(\w+)\] W\[r ?(\d+)=(\w+)\]\[\d+\] R\[r ?(\d+)=(\w+)\] R\[r ?(\d+)=(\w+)\] inst=\[(\w+)\]").unwrap();

        for line in reader.lines() {
            if let Ok(log_line) = &line {
                if let Some(caps) = rocket_re.captures(log_line) {
                    let ops = vec![
                        OpLog::RegOp(RegOp{op_type: OpType::Write, name: caps[2].to_string(), value: u64::from_str_radix(&caps[3], 16).unwrap()}),
                        OpLog::RegOp(RegOp{op_type: OpType::Read, name: caps[4].to_string(), value: u64::from_str_radix(&caps[5], 16).unwrap()}),
                        OpLog::RegOp(RegOp{op_type: OpType::Read, name: caps[6].to_string(), value: u64::from_str_radix(&caps[7], 16).unwrap()})
                    ];

                    trace.push(TraceLog {
                        pc: u64::from_str_radix(&caps[1], 16).unwrap(),
                        inst: u64::from_str_radix(&caps[8], 16).unwrap(),
                        ops,
                    });
                }
            }
        }
        Ok(trace)
    }
}

pub struct BoomExecTrace;

impl ExecTraceParser for BoomExecTrace 
{
    fn new() -> Self {
        BoomExecTrace {}
    }
    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> {
        let mut trace = Vec::<TraceLog>::new();
        // TODO
        Ok(trace)
    }
}

pub struct CVA6ExecTrace;

impl ExecTraceParser for CVA6ExecTrace 
{
    fn new() -> Self {
        CVA6ExecTrace {}
    }
    fn parse(&self, workdir: &str) -> Result<Vec<TraceLog>, Error> {
        let trace = Vec::<TraceLog>::new();
        // TODO
        Ok(trace)
    }
}