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
    executors::{command::CommandConfigurator},
    inputs::{HasTargetBytes, Input},
    Error,
};
use libafl_bolts::{
        AsMutSlice, AsSlice,
        ownedref::OwnedMutSlice
};

use std::assert;
use std::path::Path;
use std::process::{Child, Command};
use std::os::unix::fs;
use std::time::Duration;
use std::env;
use std::process::Stdio;
use rand::Rng;
use std::io::ErrorKind;
use libpresifuzz_riscv::dasm::RiscvInstructions;
use libpresifuzz_riscv::elf::ELF;
use std::io::Read;
use wait_timeout::ChildExt;

#[cfg(feature = "root_snapshot")]
use subprocess::Exec;
#[cfg(feature = "root_snapshot")]
use subprocess::Redirection;

#[cfg(feature = "debug")]
use color_print::cprintln;

extern crate yaml_rust;

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}


#[derive(Clone, Debug)]
pub struct SimvCommandConfigurator<'a> {
    workdir                   : String,
    vcs_args                  : String,
    plus_args                 : String,
    coverage_metrics          : String,
    coverage_directory        : String,
    reset_coverage_before_use : bool,
    system_timeout_s          : u64,
    vcs_timeout               : String,
    testcase_buf              : OwnedMutSlice<'a, u8>,
    shm_id                    : String,
    seed                      : u32,
}

impl<'a> CommandConfigurator for SimvCommandConfigurator<'a> {

    fn spawn_child<I: Input + HasTargetBytes>(&mut self, input: &I) -> Result<Child, Error> {

        // clean old files if any
        let old_log = "testcase_state.log"; 
        if let Ok(_metadata) = std::fs::metadata(&old_log) {
            let _ = std::fs::remove_file(&old_log);
        }

        if self.seed != 0 {
            let mut rng = rand::thread_rng();

            self.seed = rng.gen_range(0..100000);
        }
        
        #[cfg(feature = "debug")]
        cprintln!("<green>[INFO]</green> Running simv with seed {} ...", self.seed);

        // Simv Command Executor prepares simv inputs and start simv with proper arguments
        // 1. Generate testcase in expected format
        self.generate_testcase(input);

        // 2. args string into vec
        let mut forged_cmd_args = format!("\
            +vcs+finish+{vcs_timeout} \
            -cm {coverage_metrics} \
            -cm_dir {coverage_directory} \
            {plus_args} \
            {vcs_args} \
            ",
            plus_args = self.plus_args,
            vcs_args = self.vcs_args,
            vcs_timeout = self.vcs_timeout,
            coverage_metrics = self.coverage_metrics,
            coverage_directory = self.coverage_directory);

        if cfg!(feature = "root_snapshot")
        {
            forged_cmd_args.push_str(" +restore ");
            forged_cmd_args.push_str(&format!(" +ntb_random_reseed={} ", self.seed));
        } else {
            forged_cmd_args.push_str(&format!(" +ntb_random_seed={} ", self.seed));
        }

        let args_vec: Vec<&str> = forged_cmd_args.split(' ').collect();
        let args_v = &args_vec[0 .. args_vec.len()];
        
        #[cfg(feature = "debug")]
        println!("Executing command: {:?}", forged_cmd_args);
        #[cfg(feature = "debug")]
        println!("Executing command: {:?}", args_v);

        // 3. spawn simv
        if !cfg!(feature = "debug") {
        
            Ok(Command::new("bash")
             .arg("./run.sh")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start process"))
        } else {
            //Ok(Command::new("./simv")
            //    .args(args_v)
            Ok(Command::new("bash")
             .arg("./run.sh")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .expect("failed to start process"))
        }
    }

    fn exec_timeout(&self) -> Duration {
        Duration::from_secs(self.system_timeout_s.into())
    }
}

impl<'a> SimvCommandConfigurator<'a> {
    pub fn new_from_simv(simv: &SimvCommandConfigurator, testcase_buf: &'a mut [u8], shm_id: &'static str, seed: u32) -> SimvCommandConfigurator<'a> {

        #[cfg(feature = "debug")]
        cprintln!("<green>[INFO]</green> New simv with shm_id {} ...", shm_id);
    
        return SimvCommandConfigurator{
            // testcase_file_name: testcase_file_name,
            workdir                    : simv.workdir.to_string(),
            vcs_args                   : simv.vcs_args.to_string(),
            plus_args                  : simv.plus_args.to_string(),
            coverage_metrics           : simv.coverage_metrics.to_string(),
            coverage_directory         : simv.coverage_directory.to_string(),
            reset_coverage_before_use  : simv.reset_coverage_before_use,
            system_timeout_s           : simv.system_timeout_s,
            vcs_timeout                : simv.vcs_timeout.to_string(),
            testcase_buf               : OwnedMutSlice::from(testcase_buf),
            shm_id                     : shm_id.to_string(),
            seed                       : seed,
        };
    }

    pub fn new_from_config_file(config_filename: &'static str, workdir: &str, testcase_buf: &'a mut [u8], shm_id: &'static str, seed: u32) -> SimvCommandConfigurator<'a> {

        #[cfg(feature = "debug")]
        {
            println!("Loading simv configuration from {}", config_filename);
            println!("Simv workdir directory is {}", workdir);
        }

        // parse yaml configuration file to extact:
        //   * simv arguments
        //   * simv executable name
        //   * simv version
        //   * simv ISA

        let yaml_fd = std::fs::File::open(config_filename).unwrap();
        let config: serde_yaml::Value = serde_yaml::from_reader(yaml_fd).unwrap();

        let vcs_args = config["simv"]["vcs_args"]
            .as_str()
            .unwrap_or("");

        let plus_args = config["simv"]["plus_args"]
            .as_str()
            .unwrap_or("");

        let coverage_metrics = config["simv"]["coverage_metrics"]
            .as_str()
            .unwrap_or("tgl");

        let coverage_directory = config["simv"]["coverage_directory"]
            .as_str()
            .unwrap_or("Coverage.vdb");

        let reset_coverage_before_use = config["simv"]["reset_coverage_before_use"]
            .as_bool()
            .unwrap_or(false);

        let system_timeout_s = config["simv"]["system_timeout_s"]
            .as_u64()
            .unwrap_or(0);

        let vcs_timeout = config["simv"]["vcs_timeout"]
            .as_str()
            .unwrap_or("10us");

        #[cfg(feature = "debug")]
        {
            println!("simv.vcs_args = {}", vcs_args);
            println!("simv.plus_args = {}", plus_args);
            println!("simv.vcs_timeout_s = {}", vcs_timeout);
            println!("simv.system_timeout_s = {}", system_timeout_s);
            println!("simv.coverage_directory = {}", coverage_directory);
            println!("simv.coverage_metrics = {}", coverage_metrics);
            println!("simv.reset_coverage_before_use = {}", reset_coverage_before_use);
        }

        let mut vcs_home = env::current_dir().unwrap().as_os_str().to_str().unwrap().to_string(); 
        vcs_home.push_str("/build");

        let mut src_s = vec![
            format!("{}/simv.daidir", vcs_home),
            format!("{}/csrc", vcs_home),
            format!("{}/simv", vcs_home),
            format!("{}/vc_hdrs.h", vcs_home),
            format!("{}/iram.elf", vcs_home)];
 
        let mut dst_s = vec![
            format!("{}/simv.daidir", workdir),
            format!("{}/csrc", workdir),
            format!("{}/simv", workdir),
            format!("{}/vc_hdrs.h", vcs_home),
            format!("{}/iram.elf", workdir)];

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

        let src = format!("{}/{}", vcs_home, coverage_directory);
        let dst = format!("{}/{}", workdir, coverage_directory);
        copy_dir_all(&src, &dst).expect("Unable to copy vdb folder to workdir!");

        if reset_coverage_before_use == true {
            let dst = format!("{}/Virgin_coverage.vdb", workdir);
            copy_dir_all(&src, &dst).expect("Unable to create Virgin copy of vdb folder in workdir!");
        } 
        
        return SimvCommandConfigurator{
            // testcase_file_name: testcase_file_name,
            workdir                    : workdir.to_string(),
            vcs_args                   : vcs_args.to_string(),
            plus_args                  : plus_args.to_string(),
            coverage_metrics           : coverage_metrics.to_string(),
            coverage_directory         : coverage_directory.to_string(),
            reset_coverage_before_use  : reset_coverage_before_use,
            system_timeout_s           : system_timeout_s,
            vcs_timeout                : vcs_timeout.to_string(),
            testcase_buf               : OwnedMutSlice::from(testcase_buf),
            shm_id                     : shm_id.to_string(),
            seed                       : seed,
        };
    }

    fn generate_testcase<I: Input + HasTargetBytes>(&mut self, input: &I) {

        let target = input.target_bytes();
        let buf = target.as_slice();
        let size = buf.len();

        let run_from_iram = true;

        if cfg!(feature = "input_injection") {

            self.testcase_buf.as_mut_slice()[..size].copy_from_slice(&vec![0; size]);
            self.testcase_buf.as_mut_slice()[0..4].copy_from_slice(&(0x00004297_i32).to_ne_bytes());
            self.testcase_buf.as_mut_slice()[4..8].copy_from_slice(&(0x03c28293_i32).to_ne_bytes());
            self.testcase_buf.as_mut_slice()[8..12].copy_from_slice(&(0x30529073_i32).to_ne_bytes());
            // self.testcase_buf.as_mut_slice()[12..size+12].copy_from_slice(&buf.as_slice()[..size]);

            // endianess swith here from big to little endian
            // let &mut slice = &self.testcase_buf.as_mut_slice()[12..size+12];
            for k in (0..buf.as_slice().len()).step_by(4) {
                let offset = k+12;
                if k+3 < buf.as_slice().len() {
                    self.testcase_buf.as_mut_slice()[offset]   = buf.as_slice()[k+3];
                    self.testcase_buf.as_mut_slice()[offset+1] = buf.as_slice()[k+2];
                    self.testcase_buf.as_mut_slice()[offset+2] = buf.as_slice()[k+1];
                    self.testcase_buf.as_mut_slice()[offset+3] = buf.as_slice()[k];
                }
                else if k+1 < buf.as_slice().len()  {
                    self.testcase_buf.as_mut_slice()[offset]   = buf.as_slice()[k+1];
                    self.testcase_buf.as_mut_slice()[offset+1] = buf.as_slice()[k];
                }
                else {
                    break;
                }
            }

        } else {
            assert!(Path::new("iram.elf").exists());

            let slice = input.target_bytes();
            let slice = slice.as_slice();
            let riscv_ins = RiscvInstructions::from_le(slice.to_vec());

            let mut input_filename = String::from(&self.workdir);
            input_filename.push_str("/");
            input_filename.push_str("testcase");
            input_filename.push_str(".elf");

            let mut elf_template = ELF::new("iram.elf").unwrap();
            
            elf_template.update(&riscv_ins);
            elf_template.write_elf(&input_filename).unwrap();
        }
        // std::fs::remove_file("simv_0.log").expect("simv_0.log could not be found! Please check Makefile");
        // std::fs::remove_file("simv_1.log").expect("simv_0.log could not be found! Please check Makefile");
    }

    pub fn generate_root_snapshot(&mut self) {
        // Optionnal root snapshot
        #[cfg(feature = "root_snapshot")]
        {
            let mut forged_cmd_args = format!("\
                +vcs+finish+{vcs_timeout} \
                -cm {coverage_metrics} \
                -cm_dir {coverage_directory} \
                {plus_args} \
                {vcs_args} \
                +SEED={seed}", 
                plus_args = self.plus_args,
                vcs_args = self.vcs_args,
                vcs_timeout = self.vcs_timeout,
                coverage_metrics = self.coverage_metrics,
                coverage_directory = self.coverage_directory,
                seed = self.seed);

            forged_cmd_args.push_str(&format!(" +ntb_random_seed={} ", self.seed));

            let args_vec: Vec<&str> = forged_cmd_args.split(' ').collect();
            let args_v = &args_vec[0 .. args_vec.len()];

            #[cfg(feature = "debug")]
            cprintln!("<green>[INFO]</green> generating initial snapshot...");

            let _output = Exec::cmd("./simv")
                .args(args_v)
                .stdout(Redirection::Pipe)
                .stderr(Redirection::Merge)
                .capture().unwrap()
                .stdout_str();
        }
    }
}


#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    #[test]
    fn test_simv_executor() {
    }
}
 
