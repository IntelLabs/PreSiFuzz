// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::{
    executors::{ExitKind},
    observers::{Observer},
    Error,
    inputs::{UsesInput},
};

use core::{fmt::Debug};
use serde::{Deserialize, Serialize};
use libafl_bolts::{HasLen, Named};

use std::ffi::CString;
use core::ffi::c_void;
use core::ffi::c_char;
use std::ffi::c_int;
use std::os::raw::c_ulonglong;


#[repr(C)]
enum NpiFsdbValType {
    NpiFsdbBinStrVal,
    NpiFsdbOctStrVal,
    NpiFsdbDecStrVal,
    NpiFsdbHexStrVal,
    NpiFsdbSintVal,
    NpiFsdbUintVal,
    NpiFsdbRealVal,
    NpiFsdbStringVal,
    NpiFsdbEnumStrVal,
    NpiFsdbSint64Val,
    NpiFsdbUint64Val,
    NpiFsdbObjTypeVal,
}
use crate::fsdb_observer::NpiFsdbValType::*;

// Equivalent to typedef void* npiFsdbFileHandle
type NpiFsdbFileHandle = *mut c_void;

// Equivalent to NPI_UINT64
type NpiFsdbTime = c_ulonglong;

// Equivalent to std::pair <npiFsdbTime, string>
type FsdbTimeValPair = (NpiFsdbTime, String);

// Equivalent to std::vector <fsdbTimeValPair_t>
type FsdbTimeValPairVec = Vec<FsdbTimeValPair>;

#[link(name = "npi_c", kind = "static")]
extern "C" {

    fn fsdb_open(fsdb_filename: *const c_char) -> NpiFsdbFileHandle;

    fn fsdb_sig_value_between(file_hdl: NpiFsdbFileHandle, sig_name: *const c_char, begin_time: NpiFsdbTime, end_time: NpiFsdbTime, val_type: NpiFsdbValType)  -> *mut Vec<(c_ulonglong, *const c_char)>;

    fn fsdb_close(file_hdl: NpiFsdbFileHandle) -> c_void;

    fn fsdb_init() -> c_void;
    
    fn fsdb_end() -> c_void;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FSDBItemTrace {
    pub value_name: String,
    pub value_values: Vec<u64>,
    pub min_time: u64,
    pub max_time: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FSDBLog {
    pub traces: Vec<FSDBItemTrace>,
}

pub trait ExecTraceParser {
    fn new() -> Self;
    fn parse(&self, workdir: &str) -> Result<Vec<FSDBLog>, Error> ;
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[allow(clippy::unsafe_derive_deserialize)]
pub struct FSDBObserver
{
    name: String,
    workdir: String,
    fsdb_filename: String,
    trace: Vec<FSDBLog>,
}

impl FSDBObserver 
{
    pub fn new(name: &str, workdir: &str, fsdb_filename: &str) -> Self {
        Self {
            name: name.to_string(),
            trace: Vec::<FSDBLog>::new(),
            workdir: workdir.to_string(),
            fsdb_filename: fsdb_filename.to_string(),
        }
    }

    pub fn cnt(&self) -> usize {
        self.trace.len()
    }
}


impl Named for FSDBObserver 
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl HasLen for FSDBObserver
{
    fn len(&self) -> usize {
        self.trace.len()
    }
}

impl<S> Observer<S> for FSDBObserver
where
    S: UsesInput,
{
    fn pre_exec(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn post_exec(
       &mut self,
        _state: &mut S,
        _input: &S::Input,
        _exit_kind: &ExitKind,
    ) -> Result<(), Error> {

        println!("Post Exec");
        let fsdb_file_name = CString::new(self.fsdb_filename.clone()).unwrap();
        let sig_name = CString::new("").unwrap();
        let begin_time: u64 = 5000;
        let end_time: u64 = 6000;

        unsafe {
            fsdb_init();

            let file_hdl = fsdb_open(fsdb_file_name.as_ptr());
            if file_hdl.is_null() {
                println!("Fail to read fsdb");
                return Err(Error::illegal_state("FSDB not found on systemfile!"));
            }

            println!("Fail sussessfully loaded");
            let mut vc_vec : FsdbTimeValPairVec = Vec::<FsdbTimeValPair>::new();

            println!(
                "{} between {} ~ {}:",
                sig_name.to_str().unwrap(),
                begin_time,
                end_time
            );

            let vec_ptr = fsdb_sig_value_between(
                file_hdl,
                sig_name.as_ptr(),
                begin_time,
                end_time,
                NpiFsdbBinStrVal,
            );

            if !vec_ptr.is_null()
            {
                // let vc_vec_slice = std::slice::from_raw_parts(vc_vec, (*vc_vec).len());
                for time_val in vc_vec {
                    println!(
                        "At time {}, value = '{}'",
                        time_val.0,
                        time_val.1
                        // CString::from_raw(time_val.1).to_str().unwrap()
                    );
                }
            } else {
                panic!("Unable to extract fsdb values for giving time window");
            }

            fsdb_close(file_hdl);
            fsdb_end();
        }

        Ok(())
    }
}



// TODO: Re-enable this test using vdb from open source design
#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    extern crate fs_extra;
    use libafl_bolts::prelude::StdRand;
    use libafl::prelude::BytesInput;
    use libafl::executors::{ExitKind};
    use libafl_bolts::current_time;
    use libafl::prelude::InMemoryCorpus;
    use libafl::prelude::ConstFeedback;
    use crate::fsdb_observer::{FSDBObserver};
    use libafl::prelude::StdState;
    use libafl::state::HasMaxSize;
    use libafl::observers::Observer;

    #[test]
    fn test_fsdb_observer() {

        let input = BytesInput::new(vec![1, 2, 3, 4]);

        let rand = StdRand::with_seed(current_time().as_nanos() as u64);
        let corpus = InMemoryCorpus::<BytesInput>::new();

        let mut feedback = ConstFeedback::new(true);
        let mut objective = ConstFeedback::new(false);

        let mut cva6_fsdb_observer  = FSDBObserver::new("test", "./", "./test.fsdb");

        let mut state = StdState::new(
            rand,
            corpus,
            InMemoryCorpus::<BytesInput>::new(),
            &mut feedback,
            &mut objective,
        )
        .unwrap();
        state.set_max_size(1024);

        let _ = cva6_fsdb_observer.post_exec(&mut state, &input, &ExitKind::Ok);
        println!("{:?}", cva6_fsdb_observer.trace.len())
    }
}

