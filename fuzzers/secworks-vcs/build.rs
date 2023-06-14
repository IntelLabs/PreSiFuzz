// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::io::prelude::*;

fn main() {

    let rtl_dir = PathBuf::from("./fusesoc_libraries/sha256");

    if !rtl_dir.exists() {

        assert!(Command::new("fusesoc")
                .arg("library")
                .arg("add")
                .arg("sha256")
                .arg("https://github.com/secworks/sha256")
                .status()
                .unwrap()
                .success());

        assert!(Command::new("cp")
            .arg("./tb_fuzz.sv")
            .arg("./fusesoc_libraries/sha256/src/tb/")
            .status()
            .unwrap()
            .success());
       
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open("fusesoc_libraries/sha256/sha256.core")
            .unwrap();

        if let Err(e) = writeln!(file, "  tb_fuzz:") {
            eprintln!("Couldn't write to file: {}", e);
        }
        
        if let Err(e) = writeln!(file, "    <<: *tb") {
            eprintln!("Couldn't write to file: {}", e);
        }
        
        if let Err(e) = writeln!(file, "    toplevel : tb_fuzz") {
            eprintln!("Couldn't write to file: {}", e);
        }

        assert!(Command::new("sed")
            .arg("-i")
            .arg("s|- src/tb/tb_sha256.v|- src/tb/tb_fuzz.sv|g")
            .arg("fusesoc_libraries/sha256/sha256.core")
            .status()
            .unwrap()
            .success());
    }

    
    assert!(Command::new("fusesoc")
        .arg("run")
        .arg("--build")
        .arg("--target=tb_fuzz")
        .arg("--tool=vcs")
        .arg("secworks:crypto:sha256")
        .arg("--vcs_options")
        .arg("-LDFLAGS -Wl,--no-as-needed -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog")
        .status()
        .unwrap()
        .success());

    // let args = "fusesoc run --target=tb_fuzz --tool=vcs secworks:crypto:sha256 --vcs_options '-LDFLAGS -Wl,--no-as-needed -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog' --run_options '+TESTCASE=/home/nasm/Projects/HW_Fuzzing/research.security.fuzzing.hardware-fuzzing/fuzzers/baby-rtl-fuzzer/fuzz_inputs.hex -cm tgl'";
    // let args = args.split(' ');
//
    // Command::new("fusesoc")
            // .args(args)
            // .output()
            // .expect("failed to execute process");
}
