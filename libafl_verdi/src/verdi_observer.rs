// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::{
    bolts::{tuples::Named, AsIter, HasLen, AsMutSlice, AsSlice},
    executors::{ExitKind},
    observers::{MapObserver, Observer},
    observers::{DifferentialObserver, ObserversTuple},
    Error,
    inputs::{UsesInput},
};

use core::{
    slice::from_raw_parts,
    fmt::Debug,
    slice::IterMut,
    slice::Iter,
};
use libafl::prelude::AsIterMut;
use serde::{Deserialize, Serialize};
use libc::{c_uint, c_char, c_void};
use nix::{sys::wait::waitpid,unistd::{fork, ForkResult}};
use libafl::prelude::OwnedMutSlice;

extern crate fs_extra;
use std::{
    str,
    hash::Hasher,
    ffi::{CString},
};
use std::process;
use ahash::AHasher;

type NpiCovHandle = *mut c_void;

#[link(name = "npi_c", kind = "static")]
extern "C" {

    fn vdb_cov_init(vdb_file_path: *const c_char) -> NpiCovHandle;

    fn vdb_cov_end(db: NpiCovHandle) -> c_void;

    fn update_cov_map(db: NpiCovHandle, map: *mut c_uint, map_size: c_uint, coverage_type: c_uint) -> c_void;

    fn npi_init() -> c_void;
}

/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug)]
pub struct VerdiMapObserver 
{
    name: String,
    initial: u32,
    cnt: usize,
    map: Vec<u32>, 
    workdir: String,
    metric: u32,
}

#[derive(Copy, Clone)]
pub enum VerdiCoverageMetric {
    Toggle = 5,
    Line = 4
}

impl VerdiMapObserver
{

    /// Creates a new [`VerdiMapObserver`] with the given name.
    #[must_use]
    pub fn new(name: &'static str, workdir: &String, size: usize, metric: &VerdiCoverageMetric) -> Self {

        unsafe { npi_init();}
        Self {
            name: name.to_string(),
            initial: u32::default(),
            cnt: size,
            map: Vec::<u32>::with_capacity(size),
            workdir: workdir.to_string(),
            metric: *metric as u32,
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

impl Named for VerdiMapObserver
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasLen for VerdiMapObserver
{

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl MapObserver for VerdiMapObserver
{
    type Entry = u32;

    fn get(&self, idx: usize) -> &u32 {
        self.map.get(idx).unwrap()
    }

    fn get_mut(&mut self, idx: usize) -> &mut u32 {
        // self.map.get_mut(idx).unwrap()
        &mut self.map.as_mut_slice()[idx]
    }

    fn usable_count(&self) -> usize {
        self.map.len()
        // *self.cnt.as_ref()
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
    fn initial(&self) -> u32 {
        self.initial
    }

    fn reset_map(&mut self) -> Result<(), Error> {
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.map.as_mut_slice();
        for x in map[0..cnt].iter_mut() {
            *x = initial;
        }
        Ok(())
    }

    fn to_vec(&self) -> Vec<u32> {
        self.map.as_slice().to_vec()
        // self.map.clone()
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        indexes
            .iter()
            .map(|&idx| self.get(idx))
            .filter(|&&e| e != self.initial())
            .count()
    }
}

impl<S> Observer<S> for VerdiMapObserver 
where
    S: UsesInput,
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        // self.reset_map()
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

                update_cov_map(db, pmap as *mut c_uint, self.cnt as c_uint, self.metric as c_uint);

                vdb_cov_end(db);
            }
            Ok(())
        }
}

impl<'it> AsIter<'it> for VerdiMapObserver
{
    type Item = u32;
    type IntoIter = core::slice::Iter<'it, u32>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'it> AsIterMut<'it> for VerdiMapObserver
{
    type Item = u32;
    type IntoIter = IterMut<'it, u32>;

    fn as_iter_mut(&'it mut self) -> Self::IntoIter {
        self.map.as_mut_slice().iter_mut()
    }
}

impl<OTA, OTB, S> DifferentialObserver<OTA, OTB, S> for VerdiMapObserver 
where
    OTA: ObserversTuple<S>,
    OTB: ObserversTuple<S>,
    S: UsesInput,
{
    fn pre_observe_first(&mut self, _observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_first(&mut self, _observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn pre_observe_second(&mut self, _observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_second(&mut self, _observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }
}


/// A simple observer, just overlooking the runtime of the target.
#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct VerdiShMapObserver<'a, const N: usize> 
{
    name: String,
    initial: u32,
    cnt: usize,
    map: OwnedMutSlice<'a, u32>,
    workdir: String,
    metric: u32,
}

impl<'a, const N: usize> VerdiShMapObserver<'a, N>
{
    /// Creates a new [`MapObserver`]
    ///
    /// # Note
    /// Will get a pointer to the map and dereference it at any point in time.
    /// The map must not move in memory!
    #[must_use]
    pub fn new(name: &'static str, workdir: &String, map: &'a mut [u32], metric: &VerdiCoverageMetric) -> Self {
        assert!(map.len() >= N);
        unsafe {
            npi_init();
        }
        Self {
            name: name.to_string(),
            initial: u32::default(),
            cnt: map.len(),
            map: OwnedMutSlice::from(map),
            workdir: workdir.to_string(),
            metric: *metric as u32,
        }
    }

    /// Creates a new [`VerdiMapObserver`] from a raw pointer
    ///
    /// # Safety
    /// Will dereference the `map_ptr` with up to len elements.
    #[must_use]
    pub unsafe fn from_mut_ptr(name: &'static str, workdir: &String, map_ptr: *mut u32, metric: &VerdiCoverageMetric) -> Self
    {
        npi_init();

        Self {
            name: name.to_string(),
            initial: u32::default(),
            cnt: N,
            map: OwnedMutSlice::from_raw_parts_mut(map_ptr, N),
            workdir: workdir.to_string(),
            metric: *metric as u32,
        }
    }

    /// Gets cnt as usize
    #[must_use]
    pub fn cnt(&self) -> usize {
        self.cnt
    }
    
    /// Gets map ptr
    #[must_use]
    pub fn my_map(&self) -> &[u32] {
        self.map.as_slice()
    }
    
}

impl<'a, const N: usize> Named for VerdiShMapObserver<'a, N>
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl<'a, const N:usize> HasLen for VerdiShMapObserver<'a, N>
{

    fn len(&self) -> usize {
        N
    }
}

impl<'a, const N: usize> MapObserver for VerdiShMapObserver<'a, N>
{
    type Entry = u32;

    #[inline]
    fn initial(&self) -> u32 {
        self.initial
    }

    #[inline]
    fn get(&self, idx: usize) -> &u32 {
        &self.map.as_slice()[idx]
    }

    #[inline]
    fn get_mut(&mut self, idx: usize) -> &mut u32 {
        &mut self.map.as_mut_slice()[idx]
    }

    /// Count the set bytes in the map
    fn count_bytes(&self) -> u64 {
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.map.as_slice();
        let mut res = 0;
        for x in map[0..cnt].iter() {
            if *x != initial {
                res += 1;
            }
        }
        res
    }

    fn usable_count(&self) -> usize {
        self.map.as_slice().len()
    }

    fn hash(&self) -> u64 {

        let slice = &self.map.as_slice();

        let mut hasher = AHasher::new_with_keys(0, 0);
        let ptr = slice.as_ptr() as *const u8;
        let map_size = slice.len() / core::mem::size_of::<u32>();
        unsafe {
            hasher.write(from_raw_parts(ptr, map_size));
        }
        hasher.finish()
    }

    /// Reset the map
    #[inline]
    fn reset_map(&mut self) -> Result<(), Error> {
        // Normal memset, see https://rust.godbolt.org/z/Trs5hv
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.map.as_mut_slice();
        for x in map[0..cnt].iter_mut() {
            *x = initial;
        }
        Ok(())
    }

    fn to_vec(&self) -> Vec<u32> {
        self.map.as_slice().to_vec()
    }

    /// Get the number of set entries with the specified indexes
    fn how_many_set(&self, indexes: &[usize]) -> usize {
        let initial = self.initial();
        let cnt = self.usable_count();
        let map = self.map.as_slice();
        let mut res = 0;
        for i in indexes {
            if *i < cnt && map[*i] != initial {
                res += 1;
            }
        }
        res
    }
}

impl<'a, S, const N: usize> Observer<S> for VerdiShMapObserver<'a, N> 
where
    S: UsesInput,
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        let map = self.map.as_mut_slice();
        for x in map[0..N].iter_mut() {
            *x = self.initial;
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

        let pmap = self.map.as_slice();

        let pmap = pmap.as_ptr();

        let vdb = CString::new("./Coverage.vdb").expect("CString::new failed");

        match unsafe{fork()} {
            Ok(ForkResult::Parent{child, ..}) => {
                waitpid(child, None).unwrap();
            }
            Ok(ForkResult::Child) => {
                unsafe {
                    let db = vdb_cov_init(vdb.as_ptr());
                    
                    update_cov_map(db, pmap as *mut c_uint, N as c_uint, self.metric as c_uint);

                    vdb_cov_end(db);

                    process::exit(0);
                }
            },
            Err(_) => println!("libafl_verdi failed to fork to invoke libNPI.so ..."),
        }

        Ok(())
    }
}

impl<'a, 'it, const N: usize> AsIter<'it> for VerdiShMapObserver<'a, N>
{
    type Item = u32;
    type IntoIter = Iter<'it, u32>;

    fn as_iter(&'it self) -> Self::IntoIter {
        let cnt = self.usable_count();
        self.map.as_slice()[..cnt].iter()
    }
}

impl<'a, 'it, const N: usize> AsIterMut<'it> for VerdiShMapObserver<'a, N>
{
    type Item = u32;
    type IntoIter = IterMut<'it, u32>;

    fn as_iter_mut(&'it mut self) -> Self::IntoIter {
        let cnt = self.usable_count();
        self.map.as_mut_slice()[..cnt].iter_mut()
    }
}

impl<'a, OTA, OTB, S, const N: usize> DifferentialObserver<OTA, OTB, S> for VerdiShMapObserver<'a, N>
where
    OTA: ObserversTuple<S>,
    OTB: ObserversTuple<S>,
    S: UsesInput,
{
    fn pre_observe_first(&mut self, _observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_first(&mut self, _observers: &mut OTA) -> Result<(), Error> {
        Ok(())
    }

    fn pre_observe_second(&mut self, _observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }

    fn post_observe_second(&mut self, _observers: &mut OTB) -> Result<(), Error> {
        Ok(())
    }
}



