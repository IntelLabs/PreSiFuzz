// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{
    fmt::Debug,
};
use serde::{Deserialize, Serialize};
use libafl::{
    bolts::tuples::Named,
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    inputs::Input,
    observers::{ObserversTuple},
    state::HasClientPerfMonitor,
    Error,
    feedbacks::Feedback
};
use crate::verdi_observer::VerdiObserver;
extern crate fs_extra;
use fs_extra::dir::copy;
use std::fs;

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerdiFeedback {
    history: Vec<u8>,
    name: String,
    id: u32,
    outdir: String,
}

impl<I, S> Feedback<I, S> for VerdiFeedback
where
    I: Input,
    S: HasClientPerfMonitor,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &I,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<I>,
        OT: ObserversTuple<I, S>,
    {
        let observer = observers.match_name::<VerdiObserver>(self.name()).unwrap();

        let capacity = observer.cnt() as usize;
        let mut interesting : bool = false;

        let o_map = observer.map();

        for (i, item) in o_map.iter().enumerate().take(capacity) {
            if self.history[i] < *item {
                interesting = true; 
                break;
            }
        }

        println!("is Interesting? {:?}", interesting);
        if interesting {
            self.history = observer.map().clone().to_vec();

            // let's backup the results into a unique directory
            let mut options = fs_extra::dir::CopyOptions::new();
            options.content_only = true;

            let mut backup_dir_name = self.outdir.to_string();

            backup_dir_name.push_str(&format!("../backup_{}", self.id));

            let new_outdir = backup_dir_name.clone();
            fs::create_dir(new_outdir)?;

            let new_outdir = backup_dir_name.clone();
            let ret = copy(&self.outdir, new_outdir, &options);
            if let Err(e) = ret { 
                return Err(Error::illegal_state(format!("{:?}", e))) 
            }

            self.id += 1;
        }
        Ok(interesting)
    }

    /// Append to the testcase the generated metadata in case of a new corpus item
    #[inline]
    fn append_metadata(&mut self, _state: &mut S, _testcase: &mut Testcase<I>) -> Result<(), Error> {
        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &I) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for VerdiFeedback {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl VerdiFeedback {
    /// Creates a new [`VerdiFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, capacity: usize, outdir: &String) -> Self {
        let mut map = Vec::<u8>::with_capacity(capacity);
        for _i in 0..capacity {
            map.push(0);
        }
        Self {
            name: name.to_string(),
            history: map,
            id: 0,
            outdir: outdir.to_string(),
        }
    }
}

