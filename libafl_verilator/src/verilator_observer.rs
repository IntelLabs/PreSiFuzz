// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

#[cfg(not(feature = "tui"))]
use libafl::{
    bolts::{tuples::Named, AsIter, HasLen},
    executors::ExitKind,
    observers::{MapObserver, Observer},
    Error,
};
use std::str;
use core::{
    fmt::Debug,
};
// use std::path::Path;

use ahash::AHasher;
use serde::{Deserialize, Serialize};
use hashbrown::{hash_map::Entry, HashMap};

use libc::{c_uint, c_char, c_void};
extern crate fs_extra;
use fs_extra::dir::copy;
use std::fs;
use std::ffi::CString;

use core::{
    hash::Hasher,
    iter::Flatten,
    marker::PhantomData,
    slice::{from_raw_parts, Iter, IterMut},
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
    initial: usize,
    cnt: usize,
    map: Vec<usize>
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
            map: Vec::<usize>::with_capacity(size)
        }
    }

    /// Get a list ref
    #[must_use]
    pub fn map(&self) -> &Vec<usize> {
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
            
            // let mut covered = 0;
            // let mut coverable = 0;
            // let mut points = HashMap::<Vec<u8>, usize>::new();

            // let i = 0;
            for line in contents.as_str().split('\n') {
                // if line.as_bytes()[0] != b'C' {
                    // continue;
                // }
                // let mut secspace = 3;
                // for i in 0..(line.len() + 1) {
                    // if line.as_bytes()[i] == b'\'' && line.as_bytes()[i + 1] == b' ' {
                        // secspace = i;
                        // break;
                    // }
                // }
                // let point = &line.as_bytes()[3..(secspace - 3)];
                // let hits = std::str::from_utf8(&line.as_bytes()[(secspace + 1)..line.len()]).unwrap();
                // let hits: usize = hits.parse().unwrap();

                // if point.iter().any(|&x| x == "hdl/aes_tb.sv") {
                    // continue;
                // }
                // match points.entry(point.to_vec()) {
                    // Entry::Occupied(e) => {
                        // let idx = *e.get();
                        // points[idx] = hits;
                    // }
                    // Entry::Vacant(e) => {
                        // e.insert(self.map.len());
                        // points.push(hits);
                    // }
                // }
                // points.update([(point, hits)].iter().cloned().collect::<HashMap<Vec<u8>, usize>>());
                // points.update([(point, hits)].iter().cloned().collect::<HashMap<Vec<u8>, usize>>());
            // }
            // covered = 0;
            // for (point, hits) in &points {
                // if hits as &u32 > 2 {
                    // covered += 1;
                // }
            // }
            // coverable = *(&points.len() as &usize);
            // let score = (covered/coverable)*100;

                //Starting here, the code comes from https://github.com/AFLplusplus/LibAFL/pull/966
                if line.len() > 1 && line.as_bytes()[0] == b'C' {
                    let mut separator = line.len();
                    for &entry in line.as_bytes().iter().rev() {
                        if entry == b' ' {
                            break;
                        }
                        separator -= 1;
                    }
                    let (name, count) = line.as_bytes().split_at(separator);
                    let name = Vec::from(&name[3..(name.len() - 2)]); // "C '...' "
                    let count: usize = std::str::from_utf8(count)
                        .map_err(|_| Error::illegal_state("Couldn't parse the coverage count value!"))?
                        .parse()?;

                    let count : usize = (count).try_into().unwrap();
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
    type Entry = usize;

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

    fn initial_mut(&mut self) -> &mut <Self as MapObserver>::Entry { 
        &mut self.initial
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
            .into_iter()
            .map(|&idx| self.get(idx))
            .filter(|&&e| e != self.initial())
            .count()
    }
}

impl<'it> AsIter<'it> for VerilatorObserver {
    type Item = usize;
    type IntoIter = core::slice::Iter<'it, Self::Item>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.map.iter()
    }
}
