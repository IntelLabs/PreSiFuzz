// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::fs;
use std::thread;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cva6");
    println!("cargo:warning=INFO: Building cva6-vcs-fuzzer");

    let riscv_env = env::var("RISCV").unwrap_or_else(|_| "".to_string());
    assert!(!riscv_env.is_empty(), "The env variable 'RISCV' is not set");

    let fuzzer_root = env::current_dir().unwrap().display().to_string();
    let root_project = format!("{}/cva6", fuzzer_root);

    let rtl_dir = PathBuf::from("cva6");
    if !rtl_dir.exists() {
        println!("cargo:warning=INFO: Cloning cva6 repository...");

        assert!(Command::new("git")
            .arg("clone")
            .arg("https://github.com/openhwgroup/cva6.git")
            .arg(rtl_dir.as_os_str())
            .status()
            .unwrap()
            .success());

        std::env::set_current_dir(&root_project).expect("Unable to change into cva6 directory");

        println!("cargo:warning=INFO: cheking out to a fixed commit...");
        assert!(Command::new("git")
            .arg("checkout")
            .arg("2700d14471963909923bdcfa1b013b4d1b30e567")
            .status()
            .unwrap()
            .success());


        println!("cargo:warning=INFO: updating submodules");
        assert!(Command::new("git")
                .arg("submodule")
                .arg("update")
                .arg("--init")
                .arg("--recursive")
                .status()
                .unwrap()
                .success());

        println!("cargo:warning=INFO: Applying cva6.patch...");
        assert!(Command::new("git")
            .arg("apply")
            .arg(format!("{}/cva6.patch", fuzzer_root))
            .status()
            .unwrap()
            .success());

        let riscv_sim_build_dir = format!("{}/verif/core-v-verif/vendor/riscv/riscv-isa-sim/build", &root_project);
        fs::create_dir_all(&riscv_sim_build_dir).expect("Failed to create riscv_sim_build_dir");
        std::env::set_current_dir(&riscv_sim_build_dir).expect("Unable to change into core-v-verif/vendor/riscv/riscv-isa-sim/build directory");

        let prefix = format!("{}/install", riscv_sim_build_dir);
        fs::create_dir_all(&prefix).expect("Failed to create prefix directory");
        println!("cargo:warning=INFO: Configuring riscv-isa-sim with prefix: {}", prefix);

        assert!(Command::new("bash")
            .arg("-c")
            .arg(format!("../configure --prefix={}", prefix))
            .status()
            .unwrap()
            .success());

        let nproc = thread::available_parallelism().unwrap().get().to_string();

        assert!(Command::new("make")
            .arg("-j")
            .arg(&nproc)
            .arg("install-config-hdrs")
            .arg("install-hdrs")
            .arg("libfesvr.so")
            .status()
            .unwrap()
            .success());


        // Set up environment variables for the make command
        let mut cmd = Command::new("make");
        cmd.arg("-C")
           .arg(&root_project)
           .arg("vcs_build")
           .arg("target=cv64a6_imafdc_sv39")
           .arg("top_level=ariane_tb")
           .arg(format!("flist={}/core/Flist.cva6", &root_project))
           .env("RISCV", &riscv_env)
           .env("ROOT_PROJECT", &root_project)
           .env("RTL_PATH", &root_project)
           .env("CVA6_HOME_DIR", &root_project)
           .env("TB_PATH", format!("{}/verif/tb/core", &root_project))
           .env("TESTS_PATH", format!("{}/verif/tests", &root_project))
           .env("LIBRARY_PATH", format!("{}/lib", &riscv_env))
           .env("LD_LIBRARY_PATH", format!("{}/lib:{}", &riscv_env, env::var("LD_LIBRARY_PATH").unwrap_or_default()))
           .env("C_INCLUDE_PATH", format!("{}/include", &riscv_env))
           .env("CPLUS_INCLUDE_PATH", format!("{}/include", &riscv_env));

        // Auto-detect RISC-V tool name prefix
        let cv_sw_prefix = Command::new("bash")
            .arg("-c")
            .arg(format!("ls -1 {}/bin/riscv* | head -n 1 | rev | cut -d '/' -f 1 | cut -d '-' -f 2- | rev", &riscv_env))
            .output()
            .unwrap();
        let cv_sw_prefix = String::from_utf8(cv_sw_prefix.stdout).unwrap().trim().to_string() + "-";

        cmd.env("CV_SW_PREFIX", &cv_sw_prefix)
           .env("RISCV_CC", format!("{}/bin/{}gcc", &riscv_env, cv_sw_prefix))
           .env("RISCV_OBJCOPY", format!("{}/bin/{}objcopy", &riscv_env, cv_sw_prefix));

        // Set SPIKE related variables
        let spike_src_dir = format!("{}/verif/core-v-verif/vendor/riscv/riscv-isa-sim", &root_project);
        let spike_install_dir = format!("{}/verif/core-v-verif/vendor/riscv/riscv-isa-sim/build/install", &root_project);

        cmd.env("SPIKE_SRC_DIR", &spike_src_dir)
           .env("SPIKE_INSTALL_DIR", &spike_install_dir)
           .env("SPIKE_PATH", format!("{}/bin", spike_install_dir))
           .env("PATH", format!("{}/bin:{}", &riscv_env, env::var("PATH").unwrap_or_default()));

        println!("cargo:warning=INFO: Compiling cva6");
        assert!(cmd.status().unwrap().success());

    }
    assert!(Command::new("bash")
        .arg("-c")
        .arg(format!("cp -r {}/work-vcs {}/build", &root_project, &fuzzer_root))
        .status()
        .unwrap()
        .success());

    let mut cmd = Command::new("riscv64-unknown-elf-gcc");
    cmd.env("PATH", format!("{}/bin:{}", &riscv_env, env::var("PATH").unwrap_or_default()));
    cmd.arg("-DPREALLOCATE=1")
       .arg("-mcmodel=medany")
       .arg("-static")
       .arg("-std=gnu99")
       .arg("-O2")
       .arg("-ffast-math")
       .arg("-fno-common")
       .arg("-fno-builtin-printf")
       .arg("-fno-tree-loop-distribute-patterns")
       .arg("-static")
       .arg("-nostdlib")
       .arg("-nostartfiles")
       .arg("-lm")
       .arg("-lgcc")
       .arg("-T").arg(format!("{}/meta/testcase.ld", &fuzzer_root))
       .arg("-o").arg(format!("{}/build/iram.elf", &fuzzer_root))
       .arg(format!("{}/meta/testcase.S", &fuzzer_root));

    assert!(cmd.status().unwrap().success());

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
    println!("cargo:rustc-link-lib=npi_c");
    println!("cargo:rustc-link-lib=static=npi_c");

}

