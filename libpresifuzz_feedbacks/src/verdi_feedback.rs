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
use libafl_bolts::AsSlice;
use libafl::monitors::{UserStats, UserStatsValue, AggregatorOps};
use libafl::events::{Event};
use std::process::Command;
use std::path::Path;
use libafl::inputs::Input;

use libpresifuzz_observers::verdi_observer::VerdiShMapObserver as VerdiObserver;

/// Nop feedback that annotates execution time in the new testcase, if any
/// for this Feedback, the testcase is never interesting (use with an OR).
/// It decides, if the given [`TimeObserver`] value of a run is interesting.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VerdiFeedback<const N: usize> 
{
    history: Vec<u32>,
    name: String,
    id: u32,
    workdir: String,
    score: f32,
    save_on_new_coverage: bool,
    start_time: Duration,
}

impl<S, const N: usize> Feedback<S> for VerdiFeedback<N>
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
        if cfg!(feature = "const_true") {
            let mut backup_path = self.workdir.clone();
            backup_path.push_str(&format!("/backup_{}", self.id));
            
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: "VDB".to_string(),
                    value: UserStats::new( UserStatsValue::String( backup_path.clone()), AggregatorOps::Avg),
                    phantom: Default::default(),
                },
            )?;

            // backup the vdb folder
            assert!(Command::new("cp")
                .arg("-r")
                .arg("./Coverage.vdb")
                .arg(backup_path)
                .status()
                .unwrap()
                .success());
            
            self.id += 1;

            return Ok(true);
        } else if cfg!(feature = "const_false") {
            return Ok(false);
        }

        let observer = observers.match_name::<VerdiObserver<N>>(self.name()).unwrap();
        let capacity = observer.cnt();
        
        let mut interesting : bool = false;

        let o_map = observer.my_map().as_slice();

        'traverse_map: for (_k, item) in o_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {

            // each bit maps to one RTL signal 
            // any new bit set to 1 is interesting
            for i in 0..32 {
                let history_nth = (self.history[_k] >> i as u32) & 1 as u32;
                let item_nth = (*item >> i as u32) & 1 as u32;

                if history_nth != item_nth && item_nth == 1 {
                    // println!("{:x}:{:x} Finding new bit in {} compared to known state {}, with bit number {} set to 1",self.history[i], *item, item_nth, history_nth, i);
                    interesting = true; 
                    break 'traverse_map;
                } 
            }
        }

        self.score = (o_map[0] as f32 / o_map[1] as f32) * 100.0;
        println!("Analyzing vdb with coverage {} score at {}% for {}/{}", self.name, self.score, o_map[0], o_map[1]);

        let mut coverable: u32 = 0;
        let mut covered: u32 = 0;

        if interesting {
        
            let o_map = observer.my_map().as_slice();
            'traverse_map: for (_k, item) in o_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {

                for i in 0..32 {
                    let history_nth = (self.history[_k] >> i as u32) & 1 as u32;
                    let item_nth = (*item >> i as u32) & 1 as u32;

                    if history_nth == 0 && item_nth == 1 {
                        self.history[_k] |=  1 << i as u32;
                        covered += 1;
                    } else if history_nth == 1 {
                        covered += 1;
                    }
                    
                    coverable += 1;

                    if coverable >= o_map[1].try_into().unwrap() {
                        break 'traverse_map;
                    }
                    
                }
            }

            self.history[0] = covered;
            self.history[1] = coverable;
        
            self.score = (covered as f32 / coverable as f32) * 100.0;

            println!("Merge coverage {} score is {}% {}/{}", self.name, self.score, covered, coverable);

            // Save scrore into state
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: format!("coverage_{}", self.name()).to_string(),
                    value: UserStats::new( UserStatsValue::Ratio(covered as u64, coverable as u64), AggregatorOps::None),
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

            let mut backup_path = self.workdir.clone();
            backup_path.push_str(&format!("/coverage_{}_{}.vdb", self.name(), self.id));
            
            manager.fire(
                state,
                Event::UpdateUserStats {
                    name: "VDB".to_string(),
                    value: UserStats::new( UserStatsValue::String( backup_path.clone()), AggregatorOps::None),
                    phantom: Default::default(),
                },
            )?;

            // backup the vdb folder
            assert!(Command::new("cp")
                .arg("-r")
                .arg("./Coverage.vdb")
                .arg(backup_path)
                .status()
                .unwrap()
                .success());

            if self.save_on_new_coverage == true {
                _input.to_file(Path::new(&format!("{}.seed",self.id)))?;         
            }

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

impl<const N: usize> Named for VerdiFeedback<N> 
{
    #[inline]
    fn name(&self) -> &str {
        self.name.as_str()
    }
}

impl<const N: usize> VerdiFeedback<N> 
{
    /// Creates a new [`VerdiFeedback`], deciding if the given [`VerdiObserver`] value of a run is interesting.
    #[must_use]
    pub fn new_with_observer(name: &'static str, capacity: usize, workdir: &str) -> Self {
        let mut map = Vec::<u32>::with_capacity(capacity);
        for _i in 0..capacity {
            map.push(u32::default());
        }
        Self {
            name: name.to_string(),
            history: map,
            id: 0,
            workdir: workdir.to_string(),
            score: 0.0,
            save_on_new_coverage: false,
            start_time: current_time(),
        }
    }
}





