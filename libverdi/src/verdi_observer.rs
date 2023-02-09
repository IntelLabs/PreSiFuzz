// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::{
    bolts::{tuples::Named, AsIter, HasLen, AsMutSlice},
    executors::{ExitKind},
    impl_serdeany,
    observers::{self, MapObserver, Observer},
    observers::{DifferentialObserver, ObserversTuple},
    state::HasNamedMetadata,
    Error,
    inputs::{BytesInput, HasBytesVec, UsesInput},
    prelude::Input,
};

use core::{
    slice::from_raw_parts,
    fmt::Debug,
};

use serde::{Deserialize, Serialize};
use libc::{c_uint, c_char, c_void, c_float};

extern crate fs_extra;
use fs_extra::dir::copy;
use std::{
    str,
    path::Path,
    hash::Hasher,
    fs::{self, read_link, File},
    io::{BufRead, BufReader, Seek, SeekFrom},
    os::fd::FromRawFd,
    ffi::{CString}
};
use nix::{fcntl::OFlag, sys::stat::Mode, NixPath};
use ahash::AHasher;

type NpiCovHandle = *mut c_void;

#[link(name = "npi_c", kind = "static")]
extern "C" {

      fn vdb_cov_init(vdb_file_path: *const c_char) -> NpiCovHandle;

      fn vdb_cov_end(db: NpiCovHandle) -> c_void;

      fn update_cov_map(db: NpiCovHandle, map: *mut c_char, map_size: c_uint, coverage_type: c_uint) -> c_float;
}

/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug)]
pub struct VerdiMapObserver {
    name: String,
    initial: u32,
    cnt: usize,
    map: Vec<u32>,
    workdir: String,
    metric: u32,
    score: f32
}

#[derive(Copy, Clone)]
pub enum VerdiCoverageMetric {
    toggle = 4,
    line = 5
}

impl VerdiMapObserver {
    /// Creates a new [`VerdiMapObserver`] with the given name.
    #[must_use]
    pub fn new(name: &'static str, workdir: &String, size: usize, metric: &VerdiCoverageMetric) -> Self {

        Self {
            name: name.to_string(),
            initial: 0,
            cnt: size,
            map: Vec::<u32>::with_capacity(size),
            workdir: workdir.to_string(),
            metric: *metric as u32,
            score: 0.0
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
    
    /// Gets score as f32
    #[must_use]
    pub fn score(&self) -> f32 {
        self.score
    }

}

impl Named for VerdiMapObserver {
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasLen for VerdiMapObserver {
    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl MapObserver for VerdiMapObserver {
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

        let slice = &self.map;

        let mut hasher = AHasher::new_with_keys(0, 0);
        let ptr = slice.as_ptr() as *const u8;
        let map_size = slice.len() / core::mem::size_of::<u32>();
        unsafe {
            hasher.write(from_raw_parts(ptr, map_size));
        }
        hasher.finish()
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
            .into_iter()
            .map(|&idx| self.get(idx))
            .filter(|&&e| e != self.initial())
            .count()
    }
}

impl<S> Observer<S> for VerdiMapObserver
where
    S: UsesInput
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {

        // let's clean the map
        let initial = self.initial;
        for x in self.map.iter_mut() {
            *x = initial;
        }
        Ok(())
    }

    #[inline]
    fn post_exec(
        &mut self,
        _state: &mut S,
        _input: &S::Input,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {

        unsafe {
            let pmap = self.map.as_mut_ptr();
            self.map.set_len(self.cnt);

            let vdb = CString::new("./Coverage.vdb").expect("CString::new failed");
            let db = vdb_cov_init(vdb.as_ptr());

            self.score = update_cov_map(db, pmap as *mut c_char, self.cnt as c_uint, 5);

            vdb_cov_end(db);
        }

        Ok(())
    }
}

impl<'it> AsIter<'it> for VerdiMapObserver {
    type Item = u32;
    type IntoIter = core::slice::Iter<'it, Self::Item>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<OTA, OTB, S> DifferentialObserver<OTA, OTB, S> for VerdiMapObserver
where
    OTA: ObserversTuple<S>,
    OTB: ObserversTuple<S>,
    S: UsesInput,
{
    fn pre_observe_first(&mut self, observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_first(&mut self, observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn pre_observe_second(&mut self, observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_second(&mut self, observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }
}


