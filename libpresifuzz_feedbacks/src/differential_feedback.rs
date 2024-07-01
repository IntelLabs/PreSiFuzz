// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{fmt::Debug};
use serde::{Deserialize, Serialize};
use libafl::{
    events::EventFirer,
    executors::ExitKind,
    inputs::{UsesInput},
    observers::{ObserversTuple},
    Error,
    feedbacks::Feedback,
    state::{State},
};

use libafl_bolts::Named;

use libpresifuzz_observers::trace_observer::{ExecTraceObserver, ExecTraceParser};

use libpresifuzz_observers::trace_observer::OpLog::MemOp;
use libpresifuzz_observers::trace_observer::OpLog::RegOp;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DifferentialFeedback<R,C>
{
    ref_observer_name: String,
    core_observer_name: String,
    rf_ob: R,
    co_ob: C,
}

impl<S,R,C> Feedback<S> for DifferentialFeedback<R,C>
where
    S: UsesInput + State,
    R: ExecTraceParser,
    C: ExecTraceParser,
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
        OT: ObserversTuple<S>
    {
        let ref_observer = observers.match_name::<ExecTraceObserver<R>>(&self.ref_observer_name).unwrap();
        let core_observer = observers.match_name::<ExecTraceObserver<C>>(&self.core_observer_name).unwrap();

        if core_observer.cnt() == ref_observer.cnt() {
           return Ok(false);
        }

        for k in 0..ref_observer.cnt() {
            if ref_observer.trace()[k].pc != core_observer.trace()[k].pc{
                return Ok(false);
            }
            
            if ref_observer.trace()[k].inst != core_observer.trace()[k].inst{
                return Ok(false);
            }

            if ref_observer.trace()[k].ops.len() != core_observer.trace()[k].ops.len() {
                return Ok(false);
            }

            for j in 0..ref_observer.trace()[k].ops.len() {
                let ref_op = &ref_observer.trace()[k].ops[j];
                let core_op = &core_observer.trace()[k].ops[j];

                if *ref_op != *core_op {
                    return Ok(false);
                }
           }
        }

        Ok(true)
    }
}

impl<R,C> Named for DifferentialFeedback<R,C>
{
    #[inline]
    fn name(&self) -> &str {
        self.core_observer_name.as_str()
    }
}

impl<R,C> DifferentialFeedback<R,C>
where
    R: ExecTraceParser,
    C: ExecTraceParser,
{
    #[must_use]
    pub fn new(ref_observer_name: &'static str, core_observer_name: &'static str) -> Self {
        Self {
            core_observer_name: core_observer_name.to_string(),
            ref_observer_name: ref_observer_name.to_string(),
            rf_ob: R::new(),
            co_ob: C::new(),
        }
    }
}





