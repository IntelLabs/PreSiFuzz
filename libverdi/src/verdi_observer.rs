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

type NpiCovHandle = *mut c_void;

#[link(name = "npi_c", kind = "static")]
extern "C" {

      fn vdb_cov_init(vdb_file_path: *const c_char) -> NpiCovHandle;

      fn vdb_cov_end(db: NpiCovHandle) -> c_void;

      fn update_cov_map(db: NpiCovHandle, map: *mut c_char, map_size: c_uint, coverage_type: c_uint) -> c_uint;
}

/// A simple observer, just overlooking the runtime of the target.
#[derive(Serialize, Deserialize, Debug)]
pub struct VerdiObserver {
    name: String,
    vdb: String,
    initial: u8,
    cnt: usize,
    map: Vec<u8>,
    outdir: String
}

impl VerdiObserver {
    /// Creates a new [`VerdiObserver`] with the given name.
    #[must_use]
    pub fn new(name: &'static str, vdb: &String, size: usize, outdir: &String) -> Self {
        Self {
            name: name.to_string(),
            vdb: vdb.to_string(),
            initial: 0,
            cnt: size,
            map: Vec::<u8>::with_capacity(size),
            outdir: outdir.to_string(),
        }
    }

    /// Get a list ref
    #[must_use]
    pub fn map(&self) -> &Vec<u8> {
        self.map.as_ref()
    }

    /// Gets cnt as usize
    #[must_use]
    pub fn cnt(&self) -> usize {
        self.cnt
    }

}

impl<I, S> Observer<I, S> for VerdiObserver {
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
            let pmap = self.map.as_mut_ptr();
            self.map.set_len(self.cnt);

            let vdb = CString::new(self.vdb.clone()).expect("CString::new failed");
            let db = vdb_cov_init(vdb.as_ptr());

            update_cov_map(db, pmap as *mut c_char, self.cnt as c_uint, 5);

            vdb_cov_end(db);
        }

        Ok(())
    }
}

impl Named for VerdiObserver {
    fn name(&self) -> &str {
        &self.name
    }
}
