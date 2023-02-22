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
    inputs::{UsesInput},
    observers::{ObserversTuple},
    state::HasClientPerfMonitor,
    Error,
    feedbacks::Feedback
};
use libafl::bolts::AsSlice;
use libafl::monitors::UserStats;
use libafl::events::{Event};
use crate::verdi_observer::VerdiShMapObserver as VerdiObserver;
use num_traits::Bounded;
extern crate fs_extra;

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerdiFeedback<T, const N: usize> {
    history: Vec<T>,
    name: String,
    id: u32,
    outdir: String,
    score : f32
}

impl<S, T, const N: usize> Feedback<S> for VerdiFeedback<T, N>
where
    S: UsesInput + HasClientPerfMonitor,
    T: Bounded + Default + Copy + 'static + Serialize + serde::de::DeserializeOwned + Debug + PartialEq + std::cmp::PartialOrd + std::fmt::Display
{

    #[allow(clippy::wrong_self_convention)]
    fn is_interesting<EM, OT>(
        &mut self,
        state: &mut S,
        manager: &mut EM,
        _input: &S::Input,
        observers: &OT,
        _exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>
    {
        let observer = observers.match_name::<VerdiObserver<T, N>>(self.name()).unwrap();
        let capacity = observer.cnt();

        let mut interesting : bool = false;

        let o_map = observer.my_map().as_slice();

        for (i, item) in o_map.iter().enumerate().take(capacity) {
            if self.history[i] < *item {
                interesting = true; 
                break;
            }
        }

        if self.score < observer.score() {
            self.score = observer.score();

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: "coverage".to_string(),
                    value: UserStats::Float(self.score as f64),
                    phantom: Default::default(),
                },
            )?;

        }

        if interesting {

            let o_map = observer.my_map().as_slice();
            for (i, item) in o_map.iter().enumerate().take(capacity) {
                if self.history[i] < *item {
                    self.history[i] = *item;
                }
            }

            self.id += 1;
        }
        Ok(interesting)
    }

    #[inline]
    fn append_metadata(
        &mut self,
        _state: &mut S,
        _testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> {
        Ok(())
    }

    /// Discard the stored metadata in case that the testcase is not added to the corpus
    #[inline]
    fn discard_metadata(&mut self, _state: &mut S, _input: &S::Input) -> Result<(), Error> {
        Ok(())
    }
}

impl<T, const N: usize> Named for VerdiFeedback<T, N> {
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl<T: Default, const N: usize> VerdiFeedback<T, N> {
    /// Creates a new [`VerdiFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, capacity: usize, outdir: &String) -> Self {
        let mut map = Vec::<T>::with_capacity(capacity);
        for _i in 0..capacity {
            map.push(T::default());
        }
        Self {
            name: name.to_string(),
            history: map,
            id: 0,
            outdir: outdir.to_string(),
            score: 0.0
        }
    }
}

