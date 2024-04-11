// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl_bolts::{rands::Rand};
use libafl::schedulers::minimizer::TopRatedsMetadata;

use libafl::{
    corpus::{Corpus, CorpusId, Testcase},
    feedbacks::{MapFeedbackMetadata},
    inputs::UsesInput,
    observers::ObserversTuple,
    schedulers::{RemovableScheduler, Scheduler},
    state::{HasCorpus, HasMetadata, HasRand, UsesState},
    Error,
};
use rand::{thread_rng};
use rand::prelude::SliceRandom;

/// The [`HWMinimizerScheduler`] employs a genetic algorithm to compute a subset of the
/// corpus that exercise all the requested features (e.g. all the coverage seen so far)
/// prioritizing [`Testcase`]`s` using [`TestcaseScore`]
#[derive(Debug, Clone)]
pub struct HWMinimizerScheduler<CS> {
    base: CS,
    counter: u32,
    favored: Vec<CorpusId>,
    next: usize,
}

impl<CS> UsesState for HWMinimizerScheduler<CS>
where
    CS: UsesState,
{
    type State = CS::State;
}

impl<CS> RemovableScheduler for HWMinimizerScheduler<CS>
where
    CS: RemovableScheduler,
    CS::State: HasCorpus + HasMetadata + HasRand,
{
    /// Replaces the testcase at the given idx
    fn on_replace(
        &mut self,
        state: &mut CS::State,
        idx: CorpusId,
        testcase: &Testcase<<CS::State as UsesInput>::Input>,
    ) -> Result<(), Error> {
        self.base.on_replace(state, idx, testcase)?;
        self.update_favored(state, idx)
    }

    /// Removes an entry from the corpus
    fn on_remove(
        &mut self,
        _state: &mut CS::State,
        _idx: CorpusId,
        _testcase: &Option<Testcase<<CS::State as UsesInput>::Input>>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

impl<CS> Scheduler for HWMinimizerScheduler<CS>
where
    CS: Scheduler,
    CS::State: HasCorpus + HasMetadata + HasRand,
{
    /// Called when a [`Testcase`] is added to the corpus
    fn on_add(&mut self, state: &mut CS::State, idx: CorpusId) -> Result<(), Error> {
        self.base.on_add(state, idx)?;
        
        if self.counter == 10 {
            let _ = self.update_favored(state, idx);
            self.counter = 0;
        }
        self.counter+=1;
        Ok(())
    }

    /// An input has been evaluated
    fn on_evaluation<OT>(
        &mut self,
        state: &mut Self::State,
        input: &<Self::State as UsesInput>::Input,
        observers: &OT,
    ) -> Result<(), Error>
    where
        OT: ObserversTuple<Self::State>,
    {
        self.base.on_evaluation(state, input, observers)
    }

    /// Gets the next entry
    fn next(&mut self, state: &mut CS::State) -> Result<CorpusId, Error> {
        
        #[cfg(feature = "debug")]
        println!("[HWMinimizerScheduler] ratio: {}/{}", self.favored.len(), state.corpus().count());

        if self.favored.len() > 0 && state.rand_mut().below(100) < 95 {
            let idx = self.favored[self.next];
            self.next = (self.next + 1) % self.favored.len();
            Ok(idx) 
        } else {
            let idx = self.base.next(state)?;
            Ok(idx) 
        }
    }

    /// Set current fuzzed corpus id and `scheduled_count`
    fn set_current_scheduled(
        &mut self,
        _state: &mut Self::State,
        _next_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        // We do nothing here, the inner scheduler will take care of it
        Ok(())
    }
}

impl<CS> HWMinimizerScheduler<CS>
where
    CS: Scheduler,
    CS::State: HasCorpus + HasMetadata + HasRand,
{
    /// Update the [`Corpus`] favored set using the [`HWMinimizerScheduler`]
    #[allow(clippy::unused_self)]
    #[allow(clippy::cast_possible_wrap)]
    pub fn update_favored(&mut self, state: &mut CS::State, _idx: CorpusId) -> Result<(), Error> {
        // Create a new top rated meta if not existing
        if state.metadata_map().get::<TopRatedsMetadata>().is_none() {
            state.add_metadata(TopRatedsMetadata::new());
        }

        // we "schuffle" the corpus and try to find a subset of interesting inputs
        // maximizing coverage
        let mut left_to_evaluate: Vec<_> = state.corpus().ids().collect();
        left_to_evaluate.shuffle(&mut thread_rng());

        let mut history : Vec<u32> = vec![];

        self.favored.clear();

        for idx in left_to_evaluate {
        
            #[cfg(feature = "debug")]
            println!("[HWMinimizerScheduler] Evaluating map for testcase with id {:?}", idx);

            let mut entry = state.corpus().get(idx.into())?.borrow_mut();
        
            let map_state = entry
                .metadata_map_mut()
                .get_mut::<MapFeedbackMetadata<u32>>()
                .unwrap();

            let capacity = map_state.history_map.len();

            if history.len() < capacity {
                history.resize(capacity, 0);
            }

            history[0] = map_state.history_map[0];
            history[1] = map_state.history_map[1];

            'traverse_map: for (_k, item) in map_state.history_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {

                // each bit maps to one RTL signal 
                // any new bit set to 1 is interesting
                for i in 0..32 {
                    let history_nth = (history[_k] >> i as u32) & 1 as u32;
                    let item_nth = (*item >> i as u32) & 1 as u32;

                    if history_nth != item_nth && item_nth == 1 {
                        self.favored.push(idx); 

                        'copy_map: for (_k, item) in map_state.history_map.iter().enumerate().filter(|&(_k,_)| _k>=2).take(capacity) {

                            for i in 0..32 {
                                let history_nth = (history[_k] >> i as u32) & 1 as u32;
                                let item_nth = (*item >> i as u32) & 1 as u32;

                                if history_nth == 0 && item_nth == 1 {
                                    history[_k] |=  1 << i as u32;
                                } 

                                if _k*32+i > map_state.history_map[1].try_into().unwrap() {
                                    break 'copy_map;
                                }
                            }
                        }

                        break 'traverse_map;
                    } 
                }
            }
        }

        #[cfg(feature = "debug")]
        println!("[HWMinimizerScheduler] {:?} : {} - original corpus size {}", self.favored, self.favored.len(), state.corpus().count());

        Ok(())
    }

    /// Get a reference to the base scheduler
    pub fn base(&self) -> &CS {
        &self.base
    }

    /// Get a reference to the base scheduler (mut)
    pub fn base_mut(&mut self) -> &mut CS {
        &mut self.base
    }

    pub fn new(base: CS) -> Self {
        Self {
            base,
            counter: 0,
            next: 0,
            favored: Vec::<CorpusId>::new(),
        }
    }
}
