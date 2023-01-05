// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::{
    process::{Child, Stdio},
};
use std::process::Command as pcmd;
use libafl::{
    bolts::{
        // rands::Rand,
        AsSlice
    },
    executors::command::CommandConfigurator,
    inputs::{HasTargetBytes, Input},
    Error,
};
use std::fs::File;
use std::path::Path;
use std::io::prelude::*;

// Create the executor for an in-process function with just one observer
#[derive(Debug)]
pub struct VCSExecutor
{
    pub executable: String,
    pub args: String,
    pub outdir: String
}

impl CommandConfigurator for VCSExecutor
{ 
    fn spawn_child<I: Input + HasTargetBytes>(&mut self, input: &I) -> Result<Child, Error> {

        let mut input_filename = self.outdir.clone();
        input_filename.push_str("/fuzz_input.hex");

        let mut file = File::create(input_filename)?;
        let hex_input = input.target_bytes();
        let hex_input2 = hex_input.as_slice();
        for i in 0..hex_input2.len()-1 {
            let c: char = hex_input2[i].try_into().unwrap();
            write!(file, "{}", c as char)?;
        }
       
        let dir = Path::new(self.executable.as_str()).parent().unwrap();
        std::env::set_current_dir(dir).expect("Unable to change into {dir}");

        let mut command = pcmd::new(self.executable.as_str());

        let command = command
            .args(self.args.as_str().split(' '));

        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = command.spawn().expect("failed to start process");
        
        let output = command.output().expect("failed to start process");
        println!("status: {}", String::from_utf8_lossy(&output.stdout));

        // let ten_millis = time::Duration::from_millis(30000);
        // thread::sleep(ten_millis);

        Ok(child)
    }
}


