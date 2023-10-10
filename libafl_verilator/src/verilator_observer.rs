// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(feature = "tui"))]
use libafl::{
    executors::ExitKind,
    observers::{MapObserver, Observer},
    Error,
};
use libafl_bolts::{Named, AsIter, HasLen};
use std::str;
use core::{
    fmt::Debug,
};
// use std::path::Path;
use libafl::prelude::UsesInput;

use ahash::AHasher;
use serde::{Deserialize, Serialize};

extern crate fs_extra;
use std::fs;

use core::{
    hash::Hasher,
    slice::{from_raw_parts},
};

/// Compute the hash of a slice
fn hash_slice<T>(slice: &[T]) -> u64 {
    let mut hasher = AHasher::new_with_keys(0, 0);
    let ptr = slice.as_ptr() as *const u8;
    let map_size = slice.len() / core::mem::size_of::<T>();
    unsafe {
        hasher.write(from_raw_parts(ptr, map_size));
    }
    hasher.finish()
}

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

impl<S> Observer<S> for VerilatorObserver 
where
    S: UsesInput,
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {

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
        _input: &S::Input,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {

        unsafe {
            self.map.set_len(self.cnt);
 
            let contents: String = fs::read_to_string(&self.vdb)
                .expect("Unable to open Verilator coverage file at");

            let mut idx = 0;

            for line in contents.as_str().split('\n') {

                //Starting here, the code comes from https://github.com/AFLplusplus/LibAFL/pull/966
                if line.len() > 1 && line.as_bytes()[0] == b'C' {
                    let mut separator = line.len();
                    for &entry in line.as_bytes().iter().rev() {
                        if entry == b' ' {
                            break;
                        }
                        separator -= 1;
                    }
                    let count = line.as_bytes().split_at(separator).1;
                    // let (name, count) = line.as_bytes().split_at(separator);
                    // let name = Vec::from(&name[3..(name.len() - 2)]); // "C '...' "
                    let count: usize = std::str::from_utf8(count)
                        .map_err(|_| Error::illegal_state("Couldn't parse the coverage count value!"))?
                        .parse()?;

                    let count : u32 = (count).try_into().unwrap();
                    self.map[idx] = count;
                    idx += 1;
                }
                //End here
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

impl HasLen for VerilatorObserver {
    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl MapObserver for VerilatorObserver {
    type Entry = u32;

    fn get(&self, idx: usize) -> &Self::Entry {
        self.map.get(idx).unwrap()
    }

    fn get_mut(&mut self, idx: usize) -> &mut Self::Entry {
        self.map.get_mut(idx).unwrap()
    }

    fn usable_count(&self) -> usize {
        self.map.len()
    }

    fn count_bytes(&self) -> u64 {
        self.map.iter().filter(|&&e| e != self.initial()).count() as u64
    }

    fn hash(&self) -> u64 {
        hash_slice(&self.map)
    }

    #[inline(always)]
    fn initial(&self) -> Self::Entry {
        0
    }

    fn reset_map(&mut self) -> Result<(), Error> {
        let len = self.map.len();
        self.map.clear();
        self.map.resize(len, 0);
        Ok(())
    }

    fn to_vec(&self) -> Vec<Self::Entry> {
        self.map.clone()
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        indexes
            .iter()
            .map(|&idx| self.get(idx))
            .filter(|&&e| e != self.initial())
            .count()
    }
}

impl<'it> AsIter<'it> for VerilatorObserver {
    type Item = u32;
    type IntoIter = core::slice::Iter<'it, Self::Item>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.map.iter()
    }
}
