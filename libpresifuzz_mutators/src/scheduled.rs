// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

//! The `ISAISAScheduledMutator` schedules multiple mutations internally.

// use alloc::{string::String, vec::Vec};
use core::{
    fmt::{self, Debug},
    marker::PhantomData, panic,
};

use libafl_bolts::{
    rands::Rand,
    Named,
};
use libpresifuzz_riscv::{dasm::RiscvInstructions, disas::{DisasDataTable}};

use libafl::prelude::HasBytesVec;
use libafl::prelude::HasMaxSize;

use libafl::{
    mutators::{
        MutationResult, Mutator,
    },
    state::{HasRand},
    Error,
};
use crate::ISAInput;
use crate::ISAMutatorsTuple;


/// A [`Mutator`] that composes multiple mutations into one.
pub trait ComposedByMutations<MT, S>
where
    MT: ISAMutatorsTuple<S>,
{
    /// Get the mutations
    fn mutations(&self) -> &MT;

    /// Get the mutations (mutable)
    fn mutations_mut(&mut self) -> &mut MT;
}


use crate::MutationId;


/// A [`Mutator`] scheduling multiple [`Mutator`]s for an input.
pub trait ISAScheduledMutator<I, MT, S: HasMaxSize>: ComposedByMutations<MT, S>
where
    MT: ISAMutatorsTuple<S>,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, state: &mut S, input: &I) -> u64;

    /// Get the next mutation to apply
    fn schedule(&self, state: &mut S, input: &I) -> MutationId;

    fn branch_mutator(&self, input: ISAInput, state: &mut S) -> ISAInput;

    /// New default implementation for mutate.
    /// Implementations must forward `mutate()` to this method
    fn scheduled_mutate( 
    // fn scheduled_mutate<I: HasBytesVec>(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> 
    where
        I: HasBytesVec + Clone,
        S: HasRand,
    {
        let mut isa_input = ISAInput::from_input(input); 

        let mut r = MutationResult::Skipped;
        let num = self.iterations(state, input);
        for _ in 0..num {
            let idx = self.schedule(state, input);
            let outcome = self
                .mutations_mut()
                .get_and_mutate(idx, state, &mut isa_input, stage_idx)?;
            if outcome == MutationResult::Mutated {
                r = MutationResult::Mutated;
            }
        }

        /* branch mutator */
        isa_input = self.branch_mutator(isa_input, state);

        // transform back the testcase_insns into bytes
        isa_input.unparse(input.bytes_mut());

        Ok(r)
    }

    
}

pub struct BranchMutatorConf {
    pub rate: f32, /* */
}

/// A [`Mutator`] that schedules one of the embedded mutations on each call.
pub struct StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
{
    name: String,
    mutations: MT,
    max_stack_pow: u64,
    phantom: PhantomData<(I, S)>,
    branch_mutator_conf: Option<BranchMutatorConf>,
}

impl<I, MT, S> Debug for StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StdISAScheduledMutator with {} mutations for Input type {}",
            self.mutations.len(),
            core::any::type_name::<I>()
        )
    }
}

impl<I, MT, S> Named for StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
{
    fn name(&self) -> &str {
        &self.name
    }
}

impl<I, MT, S> Mutator<I, S> for StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
    I: HasBytesVec + Clone,
{
    #[inline]
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> 
    {
        self.scheduled_mutate(state, input, stage_idx)
    }
}

impl<I, MT, S> ComposedByMutations<MT, S> for StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
    // I: HasBytesVec,
{
    /// Get the mutations
    #[inline]
    fn mutations(&self) -> &MT {
        &self.mutations
    }

    // Get the mutations (mutable)
    #[inline]
    fn mutations_mut(&mut self) -> &mut MT {
        &mut self.mutations
    }
}

impl<I, MT, S> ISAScheduledMutator<I, MT, S> for StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
    I: HasBytesVec,
{
    /// Compute the number of iterations used to apply stacked mutations
    fn iterations(&self, state: &mut S, _: &I) -> u64 {
        1 << (1 + state.rand_mut().below(self.max_stack_pow))
    }

    /// Get the next mutation to apply
    fn schedule(&self, state: &mut S, _: &I) -> MutationId {
        debug_assert!(!self.mutations().is_empty());
        state.rand_mut().below(self.mutations().len() as u64).into()
    }

    fn branch_mutator(&self, input: ISAInput, state: &mut S) -> ISAInput
    where
        S: HasRand,
    {
        match &self.branch_mutator_conf {
            Some(conf) => {
                let mut tmp_data = Vec::new();
                input.unparse(&mut tmp_data);
                let riscv_ins = RiscvInstructions::from_le(tmp_data);
                //println!("riscv_ins -> {:?}", riscv_ins);
                let mut disas_data_table = DisasDataTable::from_riscv_ins(&riscv_ins, 0x1000).unwrap();
                //println!("len: {}\n", disas_data_table.len());
                
                /*
                println!("==============");
                for (_id, addr, ins) in disas_data_table.into_iter() {
                    println!("0x{:x} {:?}", addr, ins);
                }
                */

                let instruction_lens = disas_data_table.len() as u64;
                let range = state.rand_mut().between(0, disas_data_table.len() as u64);
                for i in 0..(range as f32 *conf.rate) as usize {

                    let target_index = state.rand_mut().between(0, (instruction_lens + i as u64) - 1) as usize;
                    let target_id = disas_data_table.get_id_by_index(target_index).unwrap();

                    let cofi_index = state.rand_mut().between(0, (instruction_lens + i as u64) - 1) as usize;

                    //let cofi_type = state.rand_mut().
                    //.choose(CofiType);

                    let reg1 = state.rand_mut().between(0, 31) as u8;
                    let reg2 = state.rand_mut().between(0, 31) as u8;

                    //println!("ADD at {} to {} / {}", cofi_index, target_index, instruction_lens);

                    match state.rand_mut().between(0, 10) {
                        0 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1%8, 0, libpresifuzz_riscv::disas::CofiType::CBEQZ).unwrap();
                        },
                        1 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1%8, 0, libpresifuzz_riscv::disas::CofiType::CBNEZ).unwrap();
                        },

                        /* InstructionType::CondBranchRelativeCMP */

                        2 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BEQ).unwrap();
                        },
                        3 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BNE).unwrap();
                        },
                        4 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BLT).unwrap();
                        },
                        5 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BGE).unwrap();
                        },
                        6 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BLTU).unwrap();
                        },
                        7 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), reg1, reg2, libpresifuzz_riscv::disas::CofiType::BGEU).unwrap();
                        },

                        /* InstructionType::BranchRelativeStore */
                        8 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), 0, 0, libpresifuzz_riscv::disas::CofiType::J).unwrap();
                        },
                        9 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), 0, 0, libpresifuzz_riscv::disas::CofiType::CJ).unwrap();
                        },
                        10 => {
                            disas_data_table.add_branch_instruction_by_index(cofi_index, libpresifuzz_riscv::disas::CofiTarget::TargetID(target_id), 0, 0, libpresifuzz_riscv::disas::CofiType::JAL).unwrap();
                        },
                        _ => {panic!("???")}
                    }
                }
                

                //println!("riscv_ins -> {:?}", riscv_ins);

                /*
                println!("==============");
                for (_id, addr, ins) in disas_data_table.into_iter() {
                    println!("0x{:x} {:?}", addr, ins);
                }
                */

                let new = disas_data_table.serialize_safe().serialize();
                ISAInput::new(&new)
            }
            None => {
                input
            },
        }
    }

}

impl<I, MT, S> StdISAScheduledMutator<I, MT, S>
where
    MT: ISAMutatorsTuple<S>,
    S: HasRand + HasMaxSize,
{
    /// Create a new [`StdISAScheduledMutator`] instance specifying mutations
    pub fn new(mutations: MT) -> Self {
        StdISAScheduledMutator {
            name: format!("StdISAScheduledMutator[{}]", mutations.names().join(", ")),
            mutations,
            max_stack_pow: 7,
            phantom: PhantomData,
            branch_mutator_conf: None, 
        }
    }

    /// Create a new [`StdISAScheduledMutator`] instance specifying mutations and the maximun number of iterations
    pub fn with_max_stack_pow(mutations: MT, max_stack_pow: u64) -> Self {
        StdISAScheduledMutator {
            name: format!("StdISAScheduledMutator[{}]", mutations.names().join(", ")),
            mutations,
            max_stack_pow,
            phantom: PhantomData,
            branch_mutator_conf: None, 
        }
    }

    /// Create a new [`StdISAScheduledMutator`] instance specifying mutations and the maximun number of iterations
    pub fn with_max_stack_pow_and_branch_mutator(mutations: MT, max_stack_pow: u64, branch_mutator_conf: BranchMutatorConf) -> Self {
        StdISAScheduledMutator {
            name: format!("StdISAScheduledMutator[{}]", mutations.names().join(", ")),
            mutations,
            max_stack_pow,
            phantom: PhantomData,
            branch_mutator_conf: Some(branch_mutator_conf), 
        }
    }


}

/// `SchedulerMutator` Python bindings
#[cfg(feature = "python")]
#[allow(missing_docs)]
#[allow(clippy::unnecessary_fallible_conversions)]
pub mod pybind {
    use pyo3::prelude::*;

    use super::{havoc_mutations, Debug, HavocMutationsType, StdISAScheduledMutator};
    use crate::{
        inputs::BytesInput, mutators::pybind::PythonMutator, state::pybind::PythonStdState,
    };

    #[pyclass(unsendable, name = "StdHavocMutator")]
    #[derive(Debug)]
    /// Python class for StdHavocMutator
    pub struct PythonStdHavocMutator {
        /// Rust wrapped StdHavocMutator object
        pub inner: StdISAScheduledMutator<BytesInput, HavocMutationsType, PythonStdState>,
    }

    #[pymethods]
    impl PythonStdHavocMutator {
        #[new]
        fn new() -> Self {
            Self {
                inner: StdISAScheduledMutator::new(havoc_mutations()),
            }
        }

        fn as_mutator(slf: Py<Self>) -> PythonMutator {
            PythonMutator::new_std_havoc(slf)
        }
    }

    /// Register the classes to the python module
    pub fn register(_py: Python, m: &PyModule) -> PyResult<()> {
        m.add_class::<PythonStdHavocMutator>()?;
        Ok(())
    }
}
