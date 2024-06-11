// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::fs;
use std::fs::File;

fn main() {
    println!("cargo:warning=MESSAGE");

    if Path::new("./build").is_dir() {
        assert!(fs::remove_dir_all("./build").is_ok());
    }
    assert!(fs::create_dir("./build").is_ok());
    
    let build_exec_log = File::create("build.log").expect("Failed to build.log");
    let build_exec_err = File::create("build.err").expect("Failed to build.err");

    let rtl_dir = PathBuf::from("chipyard");
    if !rtl_dir.exists() {
        println!("INFO: Builing chipyard.");

        assert!(Command::new("bash")
            .arg("build.sh")
            .stdin(Stdio::null())
            .stdout(build_exec_log)
            .stderr(build_exec_err)
            .status()
            .unwrap()
            .success())
    }

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

