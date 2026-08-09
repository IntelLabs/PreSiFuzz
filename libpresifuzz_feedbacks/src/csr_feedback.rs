// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::str;
use core::{
    fmt::Debug,
    time::Duration,
};
use serde::{Deserialize, Serialize};
use libafl::{
    corpus::Testcase,
    events::EventFirer,
    executors::ExitKind,
    inputs::{UsesInput},
    observers::{ObserversTuple},
    Error,
    feedbacks::Feedback,
    state::{State},
};

use libafl_bolts::current_time;
use libafl_bolts::Named;
use libafl::monitors::{UserStats, UserStatsValue, AggregatorOps};
use libafl::events::{Event};
use std::cmp::PartialEq;
use libpresifuzz_observers::trace_observer::CSRLog;
use libpresifuzz_observers::trace_observer::ExecTraceObserver;
use libpresifuzz_observers::trace_observer::ProcessorFuzzExecTraceObserver;

//use libafl::prelude::MapFeedbackMetadata;
//use libafl::state::HasMetadata;

#[derive(Serialize, Deserialize, Clone, Debug, Copy)]
struct CombPr {
    mstatus: u32,
    mcause: u32,
    scause: u32,
    medeleg: u32,
    mcounteren: u32,
    scounteren: u32,
}

impl CombPr {
    fn new(mstatus: u32, mcause: u32, scause: u32, medeleg: u32, mcounteren: u32, scounteren: u32) -> Self {
        Self {
            mstatus,
            mcause,
            scause,
            medeleg,
            mcounteren,
            scounteren,
        }
    }
}

impl PartialEq for CombPr {
    fn eq(&self, other: &Self) -> bool {
        self.mstatus == other.mstatus &&
        self.mcause == other.mcause &&
        self.scause == other.scause &&
        self.medeleg == other.medeleg &&
        self.mcounteren == other.mcounteren &&
        self.scounteren == other.scounteren
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy)]
struct Comb {
    mstatus: u32,
    frm: u32,
    fflags: u32,
    mcause: u32,
    scause: u32,
    medeleg: u32,
    mcounteren: u32,
    scounteren: u32,
}

impl Comb {
    fn new(mstatus: u32, frm: u32, fflags: u32, mcause: u32, scause: u32, medeleg: u32, mcounteren: u32, scounteren: u32) -> Self {
        Self {
            mstatus,
            frm,
            fflags,
            mcause,
            scause,
            medeleg,
            mcounteren,
            scounteren,
        }
    }
}

impl PartialEq for Comb {
    fn eq(&self, other: &Self) -> bool {
        self.mstatus == other.mstatus &&
        self.frm == other.frm &&
        self.fflags == other.fflags &&
        self.mcause == other.mcause &&
        self.scause == other.scause &&
        self.medeleg == other.medeleg &&
        self.mcounteren == other.mcounteren &&
        self.scounteren == other.scounteren
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy)]
struct CombF {
    mstatus_bits: u32,
    frm: u32,
    fflags: u32,
}

impl CombF {
    fn new(mstatus: u32, frm: u32, fflags: u32) -> Self {
        Self {
            mstatus_bits: (mstatus >> 13) & 3,
            frm,
            fflags,
        }
    }
}

impl PartialEq for CombF {
    fn eq(&self, other: &Self) -> bool {
        self.mstatus_bits == other.mstatus_bits &&
        self.frm == other.frm &&
        self.fflags == other.fflags
    }
}

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CSRFeedback 
{
    previous_csr: Option<CSRLog>,
    name: String,
    id: u32,
    no_transitions: u32,
    start_time: Duration,
    all_csr: Option<bool>,
    comb_priv: Vec<(u64, CombPr, CombPr)>,
    comb_func: Vec<(u64, CombF, CombF)>,
}

impl<S> Feedback<S> for CSRFeedback
where
    S: UsesInput + State,
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
        let observer = observers.match_name::<ExecTraceObserver<ProcessorFuzzExecTraceObserver>>(self.name()).unwrap();

        let mut interesting : bool = false;

        // search for new transitions considering all available CSR, requires Spike specific
        // compilation flag
        if self.all_csr.is_some() && self.all_csr.unwrap() == true {
            panic!("Processorfuzz all_csr mode is not yet implemented");
        } else {

            for trace_line in observer.trace() {

                if trace_line.csr.is_none() {
                    continue;
                }

                let csr = trace_line.csr.unwrap();

                // just a quick check to prune testcase achieving same csr states than previous one
                if self.previous_csr.is_none() || (self.previous_csr.is_some() && self.previous_csr.unwrap() != csr) {

                    // if first testcase, just update previous_csr
                    if self.previous_csr.is_none() {
                        self.previous_csr = Some(csr);
                    }
                    
                    let previous_csr = self.previous_csr.unwrap();

                    // let comb = Comb::new( csr.mstatus,
                                 // csr.frm,
                                 // csr.fflags,
                                 // csr.mcause,
                                 // csr.scause,
                                 // csr.medeleg,
                                 // csr.mcounteren,
                                 // csr.scounteren);
//
                    // let comb_p = Comb::new( previous_csr.mstatus,
                                   // previous_csr.frm,
                                   // previous_csr.fflags,
                                   // previous_csr.mcause,
                                   // previous_csr.scause,
                                   // previous_csr.medeleg,
                                   // previous_csr.mcounteren,
                                   // previous_csr.scounteren);
                    
                    let comb_pr = CombPr::new( csr.mstatus,
                                    csr.mcause,
                                    csr.scause,
                                    csr.medeleg,
                                    csr.mcounteren,
                                    csr.scounteren);
     
                    let comb_pr_p = CombPr::new( previous_csr.mstatus,
                                      previous_csr.mcause,
                                      previous_csr.scause,
                                      previous_csr.medeleg,
                                      previous_csr.mcounteren,
                                      previous_csr.scounteren);

                    let comb_f = CombF::new(csr.mstatus >> 13 & 3, 
                                  csr.frm, 
                                  csr.fflags);

                    let comb_f_p = CombF::new(previous_csr.mstatus >> 13 & 3, 
                                    previous_csr.frm, 
                                    previous_csr.fflags);
                    
                    let instr_t = trace_line.inst;

                    if !self.comb_priv.contains(&(instr_t, comb_pr_p, comb_pr).clone()) && comb_pr_p != comb_pr {
                        self.comb_priv.push((instr_t, comb_pr_p, comb_pr).clone());
                        self.no_transitions += 1;
                        interesting = true;
                    }
                    
                    if !self.comb_func.contains(&(instr_t, comb_f_p, comb_f)) && comb_f_p != comb_f {
                        self.comb_func.push((instr_t, comb_f_p, comb_f).clone());
                        self.no_transitions += 1;
                        interesting = true;
                    }
                }
            }
        }

        if interesting {

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: self.name().to_string(),
                    value: UserStats::new( UserStatsValue::Number(self.no_transitions.into()), AggregatorOps::None),
                    phantom: Default::default(),
                },
            )?;

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: format!("time_{}", self.name()).to_string(),
                    value: UserStats::new( UserStatsValue::Number((current_time() - self.start_time).as_secs()), AggregatorOps::None),
                    phantom: Default::default(),
                },
            )?;

            self.id += 1;
        }
        
        return Ok(interesting);
    }

    #[inline]
    fn append_metadata<OT>(
        &mut self,
        _state: &mut S,
        _observers: &OT,
        _testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> 
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

impl Named for CSRFeedback
{
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl CSRFeedback
{
    /// Creates a new [`CSRFeedback`], deciding if the given [`ExecTraceObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, all_csr: Option<bool>) -> Self {
        Self {
            name: name.to_string(),
            previous_csr: None,
            id: 0,
            no_transitions: 0,
            start_time: current_time(),
            all_csr: all_csr,
            comb_priv: Vec::<(u64, CombPr, CombPr)>::new(),
            comb_func: Vec::<(u64, CombF, CombF)>::new(),
        }
    }
}

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
    use libpresifuzz_observers::trace_observer::{ExecTraceObserver, ProcessorFuzzExecTraceObserver};
    use libafl::prelude::StdState;
    use libafl::state::HasMaxSize;
    use libafl::observers::Observer;
    use crate::csr_feedback::CSRFeedback;
    use libafl::prelude::NopEventManager;
    use libafl::feedbacks::Feedback;
    use libafl_bolts::prelude::tuple_list;

    #[test]
    fn test_spike_trace_observer() {

        let input = BytesInput::new(vec![1, 2, 3, 4]);

        let rand = StdRand::with_seed(current_time().as_nanos() as u64);
        let corpus = InMemoryCorpus::<BytesInput>::new();

        let mut feedback = CSRFeedback::new_with_observer("spike_trace_observer", Some(false));
        let mut objective = ConstFeedback::new(false);

        let mut spike_trace_observer = ExecTraceObserver::<ProcessorFuzzExecTraceObserver>::new("spike_trace_observer", "./spike.log");

        let mut state = StdState::new(
            rand,
            corpus,
            InMemoryCorpus::<BytesInput>::new(),
            &mut feedback,
            &mut objective,
        )
        .unwrap();
        state.set_max_size(1024);

        let mut mgr = NopEventManager::new();

        let _ = spike_trace_observer.post_exec(&mut state, &input, &ExitKind::Ok);
        for item in spike_trace_observer.trace() {
            println!("{}", item);
        }

        let observers = tuple_list!(spike_trace_observer);
        feedback.is_interesting(&mut state, &mut mgr, &input, &observers, &ExitKind::Ok); 
    }
}


