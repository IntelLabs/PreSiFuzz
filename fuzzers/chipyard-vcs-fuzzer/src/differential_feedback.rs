// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
#![cfg_attr(
    test,
    deny(
        dead_code,
        unreachable_code
    )
)]
#![allow(dead_code, unreachable_code, unused_variables, unused_mut)]

use core::fmt::Debug;
use libafl_bolts::Named;
use libafl::{
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    feedbacks::Feedback,
    inputs::{UsesInput},
    observers::ObserversTuple,
    state::State,
    Error,
};
use serde::{Deserialize, Serialize};
use std::str;
extern crate fs_extra;

#[cfg(feature = "debug")]
use color_print::cprintln;

use std::fmt::Write;
use std::{
    fs,
};

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DifferentialFeedback {
    first_name: String,
    name: String,
    counter: u32,
}

impl<S> Feedback<S> for DifferentialFeedback
where
    S: UsesInput + State 
{
    #[allow(clippy::wrong_self_convention)]
    #[allow(dead_code)]
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

        #[cfg(feature = "debug")]
        cprintln!("<red>[WARNING]</red> Skipping trace comparison feedback because it is not implemented for Rocketchip ...");
        return Ok(false);

        //TODO: implement differential testing for Spike
    }

    #[inline]
    fn append_metadata<OT>(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        _testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> 
    where
        OT: ObserversTuple<S>
    {
        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }
}

impl Named for DifferentialFeedback {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl DifferentialFeedback {
    /// Creates a new [`DifferentialFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(
        name: &'static str,
        first_name: &'static str,
    ) -> Self {
        Self {
            first_name: first_name.to_string(),
            name: name.to_string(),
            counter: 0,
        }
    }
}
