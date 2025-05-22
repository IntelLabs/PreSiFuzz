// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{
    fmt::Debug,
};
use serde::{Deserialize, Serialize};
use libafl::{
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    inputs::{UsesInput},
    observers::{ObserversTuple},
    state::State,
    Error,
    feedbacks::Feedback
};
use libafl_bolts::Named;
use crate::verilator_observer::VerilatorObserver;
extern crate fs_extra;

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerilatorFeedback {
    history: Vec<u32>,
    name: String,
    id: usize,
    outdir: String,
}

impl<S> Feedback<S> for VerilatorFeedback
where
    S: UsesInput + State,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        _state: &mut S,
        _manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        let observer = observers.match_name::<VerilatorObserver>(self.name()).unwrap();

        let capacity = observer.cnt();
        let mut interesting : bool = false;

        let o_map = observer.map();

        for (i, item) in o_map.iter().enumerate().take(capacity) {
            if self.history[i] < *item {
                interesting = true; 
                break;
            }
        }

        if interesting {
            self.history = observer.map().clone().to_vec();
            self.id += 1;
        }
        Ok(interesting)
    }

    /// Append to the testcase the generated metadata in case of a new corpus item
    #[inline]
    fn append_metadata<OT>(&mut self, _state: &mut S, _observers: &OT ,_testcase: &mut Testcase<S::Input>) -> Result<(), Error> 
    where
        OT: ObserversTuple<S>,
    {
        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for VerilatorFeedback {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl VerilatorFeedback {
    /// Creates a new [`VerilatorFeedback`], deciding if the given [`VerilatorObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, capacity: usize, outdir: &String) -> Self {
        let mut map = Vec::<u32>::with_capacity(capacity);
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


