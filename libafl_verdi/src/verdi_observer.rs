// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::{
    bolts::{tuples::Named, AsIter, HasLen},
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
};
use libafl::prelude::AsIterMut;
use serde::{Deserialize, Serialize};
use libc::{c_uint, c_char, c_void, c_float};

extern crate fs_extra;
use std::{
    str,
    hash::Hasher,
    ffi::{CString},
};
use ahash::AHasher;
use num_traits::Bounded;
type NpiCovHandle = *mut c_void;

#[link(name = "npi_c", kind = "static")]
extern "C" {

      fn vdb_cov_init(vdb_file_path: *const c_char) -> NpiCovHandle;

      fn vdb_cov_end(db: NpiCovHandle) -> c_void;

      fn update_cov_map(db: NpiCovHandle, map: *mut c_char, map_size: c_uint, coverage_type: c_uint) -> c_float;
}

/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug)]
pub struct VerdiMapObserver<T> 
where
    T: Default + Copy + 'static + Serialize,
{
    name: String,
    initial: T,
    cnt: usize,
    map: Vec<T>,
    workdir: String,
    metric: u32,
    score: f32
}

#[derive(Copy, Clone)]
pub enum VerdiCoverageMetric {
    Toggle = 4,
    Line = 5
}

impl<T> VerdiMapObserver<T>
where
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug,
{
    /// Creates a new [`VerdiMapObserver`] with the given name.
    #[must_use]
    pub fn new(name: &'static str, workdir: &String, size: usize, metric: &VerdiCoverageMetric) -> Self {

        Self {
            name: name.to_string(),
            initial: T::default(),
            cnt: size,
            map: Vec::<T>::with_capacity(size),
            workdir: workdir.to_string(),
            metric: *metric as u32,
            score: 0.0
        }
    }

    /// Get a list ref
    #[must_use]
    pub fn map(&self) -> &Vec<T> {
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

impl<T> Named for VerdiMapObserver<T>
where
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned,
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl<T> HasLen for VerdiMapObserver<T>
where
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned,
{

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

impl<T> MapObserver for VerdiMapObserver<T>
where
    // T: Default + Copy + 'static + Serialize + Bounded + PartialEq + Debug + Serialize + serde::de::DeserializeOwned
    T: Bounded
        + PartialEq
        + Default
        + Copy
        + 'static
        + Serialize
        + serde::de::DeserializeOwned
        + Debug,
{
    type Entry = T;

    fn get(&self, idx: usize) -> &T {
        self.map.get(idx).unwrap()
    }

    fn get_mut(&mut self, idx: usize) -> &mut T {
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
    fn initial(&self) -> T {
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

    fn to_vec(&self) -> Vec<T> {
        self.map.as_slice().to_vec()
        // self.map.clone()
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        indexes
            .into_iter()
            .map(|&idx| self.get(idx))
            .filter(|&&e| e != self.initial())
            .count()
    }
}

impl<S, T> Observer<S> for VerdiMapObserver<T> 
where
    S: UsesInput,
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug + PartialEq 
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

            self.score = update_cov_map(db, pmap as *mut c_char, self.cnt as c_uint, 5);

            vdb_cov_end(db);
        }

        Ok(())
    }
}

impl<'it, T> AsIter<'it> for VerdiMapObserver<T>
where
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug,
{
    type Item = T;
    type IntoIter = core::slice::Iter<'it, Self::Item>;

    fn as_iter(&'it self) -> Self::IntoIter {
        self.map.iter()
    }
}

impl<'it, T> AsIterMut<'it> for VerdiMapObserver<T>
where
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug,
{
    type Item = T;
    type IntoIter = IterMut<'it, T>;

    fn as_iter_mut(&'it mut self) -> Self::IntoIter {
        self.map.as_mut_slice().iter_mut()
    }
}

impl<OTA, OTB, S, T> DifferentialObserver<OTA, OTB, S> for VerdiMapObserver<T> 
where
    OTA: ObserversTuple<S>,
    OTB: ObserversTuple<S>,
    S: UsesInput,
    T: Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug + PartialEq,
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


