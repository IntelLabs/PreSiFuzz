// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;

fn main() {

    let args = "fusesoc library add sha256 https://github.com/secworks/sha256";
    let args = args.split(' ');

    Command::new("fusesoc")
            .args(args)
            .output()
            .expect("failed to execute process");
    
    let args = "fusesoc run --target=tb_fuzz --tool=vcs secworks:crypto:sha256 --vcs_options '-LDFLAGS -Wl,--no-as-needed -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog' --run_options '+TESTCASE=/home/nasm/Projects/HW_Fuzzing/research.security.fuzzing.hardware-fuzzing/fuzzers/baby-rtl-fuzzer/fuzz_inputs.hex -cm tgl'";
    let args = args.split(' ');

    Command::new("fusesoc")
            .args(args)
            .output()
            .expect("failed to execute process");
}
