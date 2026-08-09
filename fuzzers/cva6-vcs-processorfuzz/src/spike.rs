// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(
    test,
    deny(
        dead_code,
        unreachable_code
    )
)]
#![allow(dead_code, unreachable_code)]


use libafl::{
    executors::command::CommandConfigurator,
    inputs::{HasTargetBytes, Input},
    Error,
};

use libafl_bolts::AsSlice;
use libpresifuzz_riscv::dasm::RiscvInstructions;
use libpresifuzz_riscv::elf::ELF;

use std::time::Duration;
use std::process::Stdio;
use std::assert;
use std::path::Path;
use std::process::{Child, Command};
use std::env;
use std::os::unix::fs;
use std::io::ErrorKind;

#[cfg(feature = "debug")]
use color_print::cprintln;

extern crate yaml_rust;

#[derive(Default, Debug)]
pub struct SpikeCommandConfigurator {
    workdir           : String,
    args              : String,
    debug_file        : String,
    system_timeout_s  : u64,
    template_file     : String,
    testcase_name     : String,
    payload_address   : u32,
}

impl CommandConfigurator for SpikeCommandConfigurator {
    
    fn spawn_child<I: Input + HasTargetBytes>(&mut self, input: &I) -> Result<Child, Error> {
        
        let old_log = "testcase.elf_spike.log"; 
        if let Ok(_metadata) = std::fs::metadata(&old_log) {
            let _ = std::fs::remove_file(&old_log);
        }

        // Spike Command Executor prepares spike inputs and start spike with proper arguments
        // 1. Generate testcase in expected format
        // #[cfg(feature = "shared_memory_testcase")]
        // self.generate_testcase_shm(input);
        // #[cfg(feature = "file_testcase")]
        self.generate_testcase_file(input);

        #[cfg(feature = "debug")]
        cprintln!("<green>[INFO]</green> Running spike ...");
        
        // 2. args string into vec
        let forged_cmd_args = format!("\
            -l --log={workdir}/testcase.elf_spike.log \
            --log-commits \
            -d --debug-cmd={workdir}/{debug_file} \
            {args} \
            {workdir}/testcase.elf", workdir = self.workdir, debug_file = self.debug_file, args = self.args);
        let args_vec: Vec<&str> = forged_cmd_args.split(' ').collect();
        let args_v = &args_vec[0 .. args_vec.len()];
        
        #[cfg(feature = "debug")]
        println!("Executing command: {:?}", forged_cmd_args);
        #[cfg(feature = "debug")]
        println!("Executing command: {:?}", args_v);

        // 3. spawn spike
        if cfg!(feature = "debug") {
            Ok(Command::new("spike")
                .args(args_v)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to start process"))
        } else {
            Ok(Command::new("spike")
                .args(args_v)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("failed to start process"))
        }


    }

    fn exec_timeout(&self) -> Duration {
        Duration::from_secs(self.system_timeout_s.into())
    }
}

impl SpikeCommandConfigurator {

    pub fn testcase_name(&self) -> &str {
        self.testcase_name.as_str()
    }
    
    pub fn set_testcase_name(&mut self, new_testcase_name: String) {
        self.testcase_name = new_testcase_name; 
    }

    pub fn get_payload_address(&self) -> u32 {
        self.payload_address
    }

    pub fn new_from_config_file(config_filename: &'static str, workdir: &str) -> SpikeCommandConfigurator {
        // parse yaml configuration file to extact:
        //   * spike arguments
        //   * spike executable name
        //   * spike version
        //   * spike ISA

        let yaml_fd = std::fs::File::open(config_filename).unwrap();
        let config: serde_yaml::Value = serde_yaml::from_reader(yaml_fd).unwrap();

        let args = config["spike"]["args"]
            .as_str()
            .unwrap_or("");

        let debug_file= config["spike"]["debug_file"]
            .as_str()
            .unwrap_or("");

        let system_timeout_s = config["spike"]["system_timeout_s"]
            .as_u64()
            .unwrap_or(0);
        
        let template_file = config["spike"]["template_file"]
            .as_str()
            .unwrap_or("iram.elf");

        #[cfg(feature = "debug")]
        {
            println!("spike.args = {}", args);
            println!("spike.debug_file = {}", debug_file);
            println!("spike.system_timeout_s = {}", system_timeout_s);
            println!("spike.template_file = {}", template_file);
        }
        
        let mut spike_home = env::current_dir().unwrap().as_os_str().to_str().unwrap().to_string(); 
        spike_home.push_str("/build");

        let src_s = vec![
            format!("{}/{}", spike_home, template_file)];
 
        let dst_s = vec![
            format!("{}/{}", workdir, template_file)];

        for i in 0..src_s.len() {
            #[cfg(feature = "debug")]
            println!("Creating symlink from {src} to {dst}", src = &src_s[i], dst = &dst_s[i]);
 
            match fs::symlink(&src_s[i], &dst_s[i]) {
                Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                    println!("No need to copy {} because file already exists", &src_s[i]);
                },
                Ok(_) => {},
                _ => { panic!("Fail to create symbolic link for simv workdir!");}
            };
        }

        let tmp_template_file = format!("{}/{}", workdir, template_file);
        println!("{}", tmp_template_file);
        let payload_address = Self::find_payload_base_address(&tmp_template_file);

        return SpikeCommandConfigurator {
            workdir           : workdir.to_string(),
            args              : args.to_string(),
            debug_file        : debug_file.to_string(),
            system_timeout_s  : system_timeout_s,
            template_file     : template_file.to_string(),
            testcase_name     : "testcase".to_string(),
            payload_address   : payload_address,
        };
    }
    
    fn generate_testcase_shm<I: Input + HasTargetBytes>(&self, _input: &I) {
        panic!("generate_testcase_shm is not yet available for Spike!");
    }

    fn find_payload_base_address(file_path: &str) -> u32 {
        let (address, _offset, _size) = ELF::find_symbol(file_path, "payload").unwrap();
        address as u32
    }

    pub fn generate_testcase_file<I: Input + HasTargetBytes>(&mut self, input: &I) {
        assert!(Path::new(&self.template_file).exists());

        let slice = input.target_bytes();
        let slice = slice.as_slice();
        let riscv_ins = RiscvInstructions::from_le(slice.to_vec());

        let mut input_filename = String::from(&self.workdir);
        input_filename.push_str("/");
        input_filename.push_str(&self.testcase_name);
        input_filename.push_str(".elf");

        let mut elf_template = ELF::new(&self.template_file).unwrap();
        
        elf_template.update(&riscv_ins);
        elf_template.write_elf(&input_filename).unwrap();
    }
}


#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    #[test]
    fn test_spike_executor() {
    }
}

