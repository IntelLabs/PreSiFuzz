// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::path::Path;
use std::env;

extern crate bindgen;
use std::path::PathBuf;

fn main() {
    println!("cargo::rerun-if-changed=src/npi_c.c");

    println!("Compiling using profile {:?}. Please condiser using test profile to fake LibNPI.so", env::var("PROFILE"));

    if env::var("PRESIFUZZ_DUMMY").is_ok() {
        cc::Build::new()
            .cpp(true) // Switch to C++ library compilation.
            .file("./src/npi_c.c")
            .flag("-DDUMMY_LIB")
            .compile("npi_c");

    } else {
        let key = "VERDI_HOME";
        let mut verdi_lib = match env::var(key) {
            Ok(val) => val,
            Err(_e) => "".to_string(),
        };

        let mut verdi_inc = verdi_lib.clone();

        if verdi_inc.is_empty() || verdi_lib.is_empty() {
            println!("The env variable 'VERDI_HOME' is not set");
            return;
        }

        verdi_lib.push_str("/share/NPI/lib/linux64");
        
        let mut npi_l1 = verdi_inc.clone();
        npi_l1.push_str("/share/NPI/L1/C/inc/");

        verdi_inc.push_str("/share/NPI/inc");

        let npi_library_path = Path::new(&verdi_lib);
        let npi_include_path = Path::new(&verdi_inc);
        let npi_l1_include_path = Path::new(&npi_l1);

        cc::Build::new()
            .cpp(true) // Switch to C++ library compilation.
            .file("./src/npi_c.c")
            .flag("-lNPI -ldl -lpthread -lrt -lz -lfsdb -lnpiL1")
            .include(npi_include_path)
            .include(npi_library_path)
            .include(npi_l1_include_path)
            .compile("npi_c");
        println!("cargo:rustc-link-lib=NPI");
        println!("cargo:rustc-link-lib=fsdb");
        println!("cargo:rustc-link-lib=npiL1");

        let key = "VERDI_HOME";
        let verdi_home = match env::var(key) {
            Ok(val) => val,
            Err(_e) => "".to_string(),
        };

        println!("cargo:rustc-link-search=native={}/share/NPI/lib/linux64", verdi_home);
    }
}

