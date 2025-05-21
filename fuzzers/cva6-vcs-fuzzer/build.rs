// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::{Path};
use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::fs;

fn main() {
    println!("cargo:warning=MESSAGE");

    assert!(fs::create_dir("./build").is_ok());

    let rtl_dir = PathBuf::from("cva6");
    if !rtl_dir.exists() {

        println!("INFO: Cloning cva6 repository..");

        assert!(Command::new("git")
            .arg("clone")
            .arg("https://github.com/openhwgroup/cva6.git")
            .arg(rtl_dir.as_os_str())
            .status()
            .unwrap()
            .success());

        let popd = env::current_dir().unwrap();

        let cva6_dir = PathBuf::from("./cva6".to_string());
        let cva6_dir = cva6_dir.as_os_str().to_str().unwrap().to_string();
        std::env::set_current_dir(&cva6_dir).expect("Unable to change into cva6 directory");

        println!("INFO: cheking out good commit");

        assert!(Command::new("git")
            .arg("checkout")
            .arg("b401ab3868d053a00779add51ea37cf3b8c98b21")
            .status()
            .unwrap()
            .success());

        println!("INFO: updating submodules");

        assert!(Command::new("git")
                .arg("submodule")
                .arg("update")
                .arg("--init")
                .arg("--recursive")
                .status()
                .unwrap()
                .success());

        assert!(Command::new("git")
            .arg("apply")
            .arg("../cva6.patch")
            .status()
            .unwrap()
            .success());

        let popd = popd.as_os_str().to_str().unwrap().to_string();
        std::env::set_current_dir(&popd).expect("Unable to change into cva6-vcs-fuzzer directory");
    }


    let binding = env::current_dir().unwrap();
    let cur_dir = binding.to_str().unwrap();
    let mut cur_dir = String::from(cur_dir);
    cur_dir.push_str("/cva6");

    env::set_var("CVA6_HOME_DIR", cur_dir.clone());

    if !Path::new("./cva6/tools/spike/lib/libfesvr.so").exists() {
        println!("INFO: building fesvr..");

        assert!(Command::new("mkdir")
            .arg("-p")
            .arg("./cva6/tools/spike/lib/")
            .status()
            .unwrap()
            .success());
    }

    if !Path::new("./cva6/tmp").is_dir() {
        assert!(Command::new("mkdir")
            .arg("./cva6/tmp")
            .status()
            .unwrap()
            .success());

        assert!(Command::new("bash")
                    .arg("-c")
                    .arg("cd ./cva6 \
                    && RISCV=$CVA6_HOME_DIR/tools/spike source ./ci/install-fesvr.sh")
                    .env("CVA6_HOME_DIR", cur_dir.clone())
                    .status()
                    .unwrap()
                    .success());
    }


    if !Path::new("./build/simv").is_file() {
        println!("INFO: building cva6..");
        
        assert!(Command::new("bash")
                    .arg("-c")
                    .arg("echo $CVA6_HOME_DIR && cd ./cva6/verif/sim/ && source ./setup-env.sh && python3 ./cva6.py --target cv32a60x --iss=vcs-testharness --iss_yaml=cva6.yaml \
                        --asm_tests $CVA6_HOME_DIR/../src/testcase.S \
                        --linker=$CVA6_HOME_DIR/../src/testcase.ld \
                        --gcc_opts='-static -mcmodel=medany -fvisibility=hidden -nostdlib -nostartfiles -g -lgcc'")
                    .stdout(Stdio::inherit())
                    .env("CVA6_HOME_DIR", cur_dir)
                    .status()
                    .unwrap()
                    .success());
    }

    println!("cargo:rerun-if-changed=cva6_project");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build");
        
    println!("INFO: creating build dir..");
    
    assert!(Command::new("bash")
                .arg("-c")
                .arg("cp -r ./cva6/work-vcs/* ./build")
                .status()
                .unwrap()
                .success());

    assert!(Command::new("bash")
                .arg("-c")
                .arg("cp ./cva6/verif/sim/out_*/directed_asm_tests/testcase.o ./build/iram.elf")
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

