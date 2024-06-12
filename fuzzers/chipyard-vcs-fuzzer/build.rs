// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path, PathBuf};
use std::env;
use std::process::{Command, Stdio};
use std::fs;
use std::fs::File;

fn main() {
    println!("cargo:warning=MESSAGE");

    if Path::new("./build").is_dir() {
        assert!(fs::remove_dir_all("./build").is_ok());
    }
    assert!(fs::create_dir("./build").is_ok());

    // Check if chipyard directory exists
    if !Path::new("chipyard").exists() {
        println!("chipyard is not present in the current directory. Downloading..");
        clone_chipyard();
    } else {
        println!("chipyard directory already exists.");
    }

    // Change directory to chipyard
    std::env::set_current_dir("chipyard").expect("Failed to change directory to chipyard");

    // Execute build-setup.sh
    run_command("./build-setup.sh", &["--skip-marshal", "--skip-firesim", "--skip-toolchain", "--skip-conda", "riscv-tools"]);

    // Source env.sh
    source_env_sh();

    // Apply patch
    run_command("git", &["apply", "../chipyard_cov.patch"]);

    // Change directory to sims/vcs/
    std::env::set_current_dir("sims/vcs/").expect("Failed to change directory to sims/vcs/");

    // Compile with make
    run_command("make", &["-j", "12"]);

    // Build the testcase
    build_testcase();

    println!("INFO: Creating build dir.");

    assert!(Command::new("bash")
                .arg("-c")
                .arg("cp -r ./chipyard/sims/vcs/* ./build")
                .status()
                .unwrap()
                .success());

    let key = "VERDI_HOME";
    let mut verdi_lib = match env::var(key) {
        Ok(val) => val,
        Err(_e) => "".to_string(),
    };
    
    if verdi_lib.is_empty() {
        println!("The env variable 'VERDI_HOME' is not set");
        return;
    }
    
    verdi_lib.push_str("/share/NPI/lib/linux64");

    println!("cargo:rustc-link-search=native=./build");
    println!("cargo:rustc-link-search=native={}", verdi_lib);
}

fn is_conda_installed() -> bool {
    Command::new("conda").output().is_ok()
}

fn run_command(command: &str, args: &[&str]) {
    let status = Command::new(command)
        .args(args)
        .status()
        .expect(&format!("Failed to execute {}", command));
    if !status.success() {
        panic!("Command {} failed with exit code {:?}", command, status.code());
    }
}

fn clone_chipyard() {
    let repo_url = "https://github.com/ucb-bar/chipyard.git";
    let branch = "1.11.0";
    let status = Command::new("git")
        .args(&["clone", "-b", branch, repo_url])
        .status()
        .expect("Failed to clone chipyard repository");

    if !status.success() {
        panic!("Failed to clone chipyard repository");
    }
}

fn source_env_sh() {
    let output = Command::new("bash")
        .arg("-c")
        .arg("source ./env.sh && env")
        .output()
        .expect("Failed to source env.sh");

    if output.status.success() {
        let new_env: Vec<(String, String)> = String::from_utf8(output.stdout)
            .expect("Failed to parse output of env.sh")
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.splitn(2, '=').collect();
                if parts.len() == 2 {
                    Some((parts[0].to_string(), parts[1].to_string()))
                } else {
                    None
                }
            })
            .collect();
        for (key, value) in new_env {
            std::env::set_var(key, value);
        }
    } else {
        panic!("Sourcing env.sh failed");
    }
}

fn build_testcase() {
    let status = Command::new("riscv64-unknown-elf-gcc")
        .args(&[
            "-DPREALLOCATE=1",
            "-mcmodel=medany",
            "-static",
            "-std=gnu99",
            "-O2",
            "-ffast-math",
            "-fno-common",
            "-fno-builtin-printf",
            "-fno-tree-loop-distribute-patterns",
            "-o", "../build/iram.elf",
            "../src/testcase.S",
            "-static",
            "-nostdlib",
            "-nostartfiles",
            "-lm",
            "-lgcc",
            "-T", "../src/test.ld"
        ])
        .status()
        .expect("Failed to build testcase");

    if !status.success() {
        panic!("Building testcase failed");
    }
}


//    let build_exec_log = File::create("build.log").expect("Failed to build.log");
//    let build_exec_err = File::create("build.err").expect("Failed to build.err");
//
//    let rtl_dir = PathBuf::from("chipyard");
//    if !rtl_dir.exists() {
//        println!("INFO: Builing chipyard.");
//
//        assert!(Command::new("bash")
//            .arg("build.sh")
//            .stdin(Stdio::null())
//            .stdout(build_exec_log)
//            .stderr(build_exec_err)
//            .status()
//            .unwrap()
//            .success())
//    }
