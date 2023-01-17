// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(feature = "tui"))]
use libafl::{
    bolts::{
        tuples::Named,
    },
    executors::ExitKind,
    observers::Observer,
    Error,
};
use std::str;
use core::{
    fmt::Debug,
};
// use std::path::Path;

// use ahash::AHasher;
use serde::{Deserialize, Serialize};

use libc::{c_uint, c_char, c_void};
extern crate fs_extra;
use fs_extra::dir::copy;
use std::fs;
use std::ffi::CString;


/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug)]
pub struct VerilatorObserver {
    name: String,
    vdb: String,
    initial: u32,
    cnt: usize,
    map: Vec<u32>
}

impl VerilatorObserver {
    /// Creates a new [`VerilatorObserver`] with the given name.
    #[must_use]
    pub fn new(name: &'static str, vdb: &String, size: usize) -> Self {
        Self {
            name: name.to_string(),
            vdb: vdb.to_string(),
            initial: 0,
            cnt: size,
            map: Vec::<u32>::with_capacity(size)
        }
    }

    /// Get a list ref
    #[must_use]
    pub fn map(&self) -> &Vec<u32> {
        self.map.as_ref()
    }

    /// Gets cnt as usize
    #[must_use]
    pub fn cnt(&self) -> usize {
        self.cnt
    }

}

impl<I, S> Observer<I, S> for VerilatorObserver {
    fn pre_exec(&mut self, _state: &mut S, _input: &I) -> Result<(), Error> {

        // let's clean the map
        let initial = self.initial;
        for x in self.map.iter_mut() {
            *x = initial;
        }
        Ok(())
    }

    fn post_exec(
        &mut self,
        _state: &mut S,
        _input: &I,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {

        unsafe {
            self.map.set_len(self.cnt);
            
            let contents: String = fs::read_to_string(self.vdb.clone())
                .expect(&format!("Unable to open Verilator coverage file at {}", &self.vdb.clone()));

            let mut idx = 0;
            for line in contents.as_str().split('\n') {
                if line.len() > 1 && line.as_bytes()[0] == b'C' {
                    let mut separator = line.len();
                    // for &entry in line.iter().rev() {
                    for &entry in line.as_bytes().iter().rev() {
                        if entry == b' ' {
                            break;
                        }
                        separator -= 1;
                    }
                    let (name, count) = line.as_bytes().split_at(separator);
                    let name = Vec::from(&name[3..(name.len() - 2)]); // "C '...' "
                    let count: u32 = std::str::from_utf8(count)
                        .map_err(|_| Error::illegal_state("Couldn't parse the coverage count value!"))?
                        .parse()?;

                    let count : u32 = (count).try_into().unwrap();
                    self.map[idx] = count; 
                    idx += 1;
                }
            }
        }

        Ok(())
    }
}

impl Named for VerilatorObserver {
    fn name(&self) -> &str {
        &self.name
    }
}

