use libafl_bolts::{AsIter, HasLen, Named};
use libafl::corpus::Testcase;
use libafl::events::EventFirer;
use libafl::executors::ExitKind;
use libafl::feedbacks::{Feedback, MinMapFeedback};
use libafl::inputs::UsesInput;
use libafl::observers::Observer;
use libafl::observers::{MapObserver, ObserversTuple};
use libafl::feedbacks::{MaxMapFeedback};
use libafl::state::{HasNamedMetadata};
use libafl::Error;
use libafl::state::{State};
use num_traits::Bounded;

use serde::{Deserialize, Serialize};

use std::fmt::Debug;
use std::marker::PhantomData;

/// Implemented by observers which detect the number of executed cycles.
pub trait CyclesExecutedObserver {
    /// The number of cycles in this execution.
    fn cycles(&self) -> u64;
}

// pub mod calibrate;

/// Map feedback which preprocesses the cycles into a map such that if a coverage point is
/// non-initial in the provided coverage map, then it is mapped into the coverage map as the number
/// of cycles for that execution. An execution is then "interesting" if it reduces the number of
/// cycles from the best so far. This may be used in place of a typical coverage map.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CyclesMapFeedback<F, CO, MO> {
    inner: F,
    cycles_initial: u64,
    cycles_obs_name: String,
    map_obs_name: String,
    proxy_name: Option<String>,
    phantom: PhantomData<(CO, MO)>,
}

impl<F, CO, MO> CyclesMapFeedback<F, CO, MO>
where
    CO: Named,
    MO: Named,
{
    fn _new(
        feedback: F,
        cycles_initial: u64,
        cycles_obs: &CO,
        map_obs: &MO,
        proxy_name: String,
    ) -> Self {
        Self {
            inner: feedback,
            cycles_initial,
            cycles_obs_name: cycles_obs.name().to_string(),
            map_obs_name: map_obs.name().to_string(),
            proxy_name: Some(proxy_name),
            phantom: PhantomData,
        }
    }
}

impl<CO, MO, S, T> CyclesMapFeedback<MinMapFeedback<CyclesMapProxy<MO>, S, u64>, CO, MO>
where
    CO: CyclesExecutedObserver + Named,
    MO: MapObserver<Entry = T> + Observer<S> + for<'a> AsIter<'a, Item = T> + std::fmt::Debug,
    S: UsesInput + HasNamedMetadata + Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    pub fn minimising(
        cycles_obs: &CO,
        map_obs: &MO,
        track_indexes: bool,
        track_novelties: bool,
    ) -> Self {
        let temp = CyclesMapProxy::new(
            // format!("cycles_{}", cycles_obs.name()),
            cycles_obs.name().to_string(),
            cycles_obs,
            map_obs,
            u64::MAX,
        );
        let feedback = MinMapFeedback::tracking(&temp, track_indexes, track_novelties);
        Self::_new(feedback, u64::MAX, cycles_obs, map_obs, temp.take_name())
    }
}

impl<CO, MO, S, T> CyclesMapFeedback<MaxMapFeedback<CyclesMapProxy<MO>, S, u64>, CO, MO>
where
    CO: CyclesExecutedObserver + Named,
    MO: MapObserver<Entry = T> + Observer<S> + for<'a> AsIter<'a, Item = T> + std::fmt::Debug,
    S: UsesInput + HasNamedMetadata + Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    pub fn minimising(
        cycles_obs: &CO,
        map_obs: &MO,
        track_indexes: bool,
        track_novelties: bool,
    ) -> Self {
        let temp = CyclesMapProxy::new(
            format!("cycles_{}", cycles_obs.name()),
            cycles_obs,
            map_obs,
            0,
        );
        let feedback = MaxMapFeedback::tracking(&temp, track_indexes, track_novelties);
        Self::_new(feedback, 0, cycles_obs, map_obs, temp.take_name())
    }
}

impl<F, CO, MO> Named for CyclesMapFeedback<F, CO, MO>
where
    F: Named,
{
    fn name(&self) -> &str {
        self.inner.name()
    }
}

fn undeserializeable<T>() -> T {
    panic!("Cannot deserialize type.")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CyclesMapProxy<MO> {
    name: String,
    #[serde(skip, default = "undeserializeable")]
    map: *const MO,
    initial: u64,
    cycles: u64,
}

impl<MO> CyclesMapProxy<MO> {
    fn new<CO: CyclesExecutedObserver>(
        name: String,
        cycles_obs: &CO,
        map_obs: &MO,
        initial: u64,
    ) -> Self {
        Self {
            name,
            map: map_obs,
            initial,
            cycles: cycles_obs.cycles(),
        }
    }

    fn take_name(self) -> String {
        self.name
    }
}

impl<MO> Named for CyclesMapProxy<MO> {
    fn name(&self) -> &str {
        &self.name
    }
}

impl<S, MO> Observer<S> for CyclesMapProxy<MO>
where
    MO: Debug,
    S: UsesInput,
{
}

impl<MO> HasLen for CyclesMapProxy<MO>
where
    MO: HasLen,
{
    fn len(&self) -> usize {
        unsafe { self.map.as_ref().unwrap_unchecked().len() }
    }
}

pub struct CycleIndexIter<'a, MO, T>
where
    MO: MapObserver<Entry = T> + for<'b> AsIter<'b, Item = T> + std::fmt::Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    map_initial: T,
    into_iter: <MO as AsIter<'a>>::IntoIter,
    initial: &'a u64,
    cycles: &'a u64,
}

impl<'it, MO, T> Iterator for CycleIndexIter<'it, MO, T>
where
    MO: MapObserver<Entry = T> + for<'b> AsIter<'b, Item = T> + std::fmt::Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    type Item = &'it u64;

    fn next(&mut self) -> Option<Self::Item> {
        self.into_iter.next().map(|e| {
            if *e == self.map_initial {
                self.initial
            } else {
                self.cycles
            }
        })
    }
}

impl<'it, MO, T> AsIter<'it> for CyclesMapProxy<MO>
where
    MO: MapObserver<Entry = T> + for<'a> AsIter<'a, Item = T> + std::fmt::Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    type Item = u64;
    type IntoIter = CycleIndexIter<'it, MO, T>;

    fn as_iter(&'it self) -> Self::IntoIter {
        CycleIndexIter {
            map_initial: unsafe { self.map.as_ref().unwrap_unchecked().initial() },
            into_iter: unsafe { self.map.as_ref().unwrap_unchecked().as_iter() },
            initial: &self.initial,
            cycles: &self.cycles,
        }
    }
}

impl<MO, T> MapObserver for CyclesMapProxy<MO>
where
    MO: MapObserver<Entry = T> + for<'a> AsIter<'a, Item = T> + std::fmt::Debug,
    T: Bounded + PartialEq + Default + Copy + Debug + 'static,
{
    type Entry = u64;

    fn get(&self, idx: usize) -> &Self::Entry {
        if unsafe {
            *self.map.as_ref().unwrap_unchecked().get(idx)
                != self.map.as_ref().unwrap_unchecked().initial()
        } {
            &self.cycles
        } else {
            &self.initial
        }
    }

    fn get_mut(&mut self, _idx: usize) -> &mut Self::Entry {
        unimplemented!("Cannot implement get_mut for CyclesMapProxy.");
    }

    fn usable_count(&self) -> usize {
        unsafe { self.map.as_ref().unwrap_unchecked().usable_count() }
    }

    fn count_bytes(&self) -> u64 {
        unsafe { self.map.as_ref().unwrap_unchecked().count_bytes() }
    }

    fn hash(&self) -> u64 {
        unimplemented!("No reason to implement hash for CyclesMapProxy.");
    }

    fn initial(&self) -> Self::Entry {
        self.initial
    }

    fn reset_map(&mut self) -> Result<(), Error> {
        Ok(())
    }

    fn to_vec(&self) -> Vec<Self::Entry> {
        (0..self.usable_count()).map(|idx| *self.get(idx)).collect()
    }

    fn how_many_set(&self, indexes: &[usize]) -> usize {
        unsafe { self.map.as_ref().unwrap_unchecked().how_many_set(indexes) }
    }
}

impl<F, CO, MO, S> Feedback<S> for CyclesMapFeedback<F, CO, MO>
where
    F: Feedback<S>,
    CO: CyclesExecutedObserver + Debug,
    MO: MapObserver + std::fmt::Debug,
    S: UsesInput + State,
{
    fn init_state(&mut self, state: &mut S) -> Result<(), Error> {
        self.inner.init_state(state)
    }

    fn is_interesting<EM, OT>(
        &mut self,
        state: &mut S,
        manager: &mut EM,
        input: &S::Input,
        observers: &OT,
        exit_kind: &ExitKind,
    ) -> Result<bool, Error>
    where
        EM: EventFirer<State = S>,
        OT: ObserversTuple<S>,
    {
        let cycles = observers.match_name::<CO>(&self.cycles_obs_name).unwrap();
        let map = observers.match_name::<MO>(&self.map_obs_name).unwrap();
        let proxy = CyclesMapProxy::new(
            self.proxy_name.take().unwrap(),
            cycles,
            map,
            self.cycles_initial,
        );
        let proxy_tuple = (proxy, ());
        let interesting =
            self.inner
                .is_interesting(state, manager, input, &proxy_tuple, exit_kind)?;
        let _ = self.proxy_name.insert(proxy_tuple.0.take_name());
        Ok(interesting)
    }

    fn append_metadata<OT>(
        &mut self,
        state: &mut S,
        observers: &OT,
        testcase: &mut Testcase<S::Input>,
    ) -> Result<(), Error> 
    where
        OT: ObserversTuple<S>
    {
        self.inner.append_metadata(state, observers, testcase)
    }

    fn discard_metadata(&mut self, state: &mut S, input: &S::Input) -> Result<(), Error> {
        self.inner.discard_metadata(state, input)
    }
}

