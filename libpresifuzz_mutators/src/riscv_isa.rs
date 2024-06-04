// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl_bolts::{
        rands::Rand, Named,
        tuples::{tuple_list},
};
use libafl::{
    // inputs::BytesInput,
    mutators::{MutationResult},
    state::{HasCorpus, HasMaxSize, HasRand},
    state::{HasMetadata},
    Error,
};
use libafl_bolts::HasLen;
use crate::ISAMutator;
use crate::ISAInput;

use libpresifuzz_riscv::instruction::Instruction;
use libpresifuzz_riscv::cpu_profile;

#[derive(Default, Debug)]
pub struct InstDeleteMutator;
impl<S> ISAMutator<S> for InstDeleteMutator
where
    S: HasRand  + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {

        if input.instructions.len() < 1 {
            return Ok(MutationResult::Skipped);
        }

        let idx = state.rand_mut().below(input.instructions.len() as u64) as usize;
        input.instructions.remove(idx);

        Ok(MutationResult::Mutated)
    }
}

impl Named for InstDeleteMutator {
    fn name(&self) -> &str {
        "InstDeleteMutator"
    }
}

impl InstDeleteMutator {
    /// Creates a a new [`LoadStoreMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[derive(Default, Debug)]
pub struct OperandMutator;
impl<S> ISAMutator<S> for OperandMutator
where
    S: HasRand  + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {

        let len: usize = input.len(); 

        if len <= 4 {
            return Ok(MutationResult::Skipped);
        }

        let idx = state.rand_mut().below(input.instructions.len() as u64) as usize;
        let mut meta = &mut input.instructions[idx]; 

        let mut inst : u32 = 0;
        let mask: u32 = meta.mask;
        inst |= mask;
        for operand in meta.operands.clone().into_iter() {

            let nb_bits = operand.1-operand.0 + 1;
            let op_max = ((1<<(1-1))-1)^((1<<nb_bits)-1);
            
            let op_value = state.rand_mut().below(op_max) as u32;

            inst |= build_operand(op_value, operand.0, operand.1);
        }
        meta.instruction = inst as u64;

        Ok(MutationResult::Mutated)
    }
}

impl Named for OperandMutator {
    fn name(&self) -> &str {
        "OperandMutator"
    }
}

impl OperandMutator {
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[derive(Default, Debug)]
pub struct AppendInstMutator;
impl<S> ISAMutator<S> for AppendInstMutator
where
    S: HasRand  + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {

        let len: usize = input.len(); 

        if len+4 >= state.max_size() {
            return Ok(MutationResult::Skipped);
        }
            
        let target_idx = state.rand_mut().below(cpu_profile::INSTRUCTIONS.len() as u64) as usize;

        let meta = &cpu_profile::INSTRUCTIONS[target_idx];

        let mut inst : u32 = 0;
        let mask: u32 = meta.mask;
        inst |= mask;
        for operand in meta.operands.clone().into_iter() {

            let nb_bits = operand.1-operand.0 + 1;
            let op_max = ((1<<(1-1))-1)^((1<<nb_bits)-1);
            
            let op_value = state.rand_mut().below(op_max) as u32;

            inst |= build_operand(op_value, operand.0, operand.1);
        }

        let mutation = Instruction{
            instruction: inst as u64,
            length: meta.length,
            mask: meta.mask,
            mmatch: meta.mmatch,
            mnemonic: meta.mnemonic.clone(),
            extension: meta.extension.clone(),
            operands: meta.operands.clone(),
        };

        // randomly insert mutant
        let idx = state.rand_mut().below(input.instructions.len() as u64) as usize;

        input.instructions.insert(idx, mutation.clone());

        Ok(MutationResult::Mutated)
    }
}

impl Named for AppendInstMutator {
    fn name(&self) -> &str {
        "AppendInstMutator"
    }
}

impl AppendInstMutator {
    /// Creates a a new [`LoadStoreMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}


#[derive(Default, Debug)]
pub struct OpcodeMutator;
impl<S> ISAMutator<S> for OpcodeMutator
where
    S: HasRand  + HasMaxSize,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {

        let len: usize = input.len(); 

        if len <= 4 {
            return Ok(MutationResult::Skipped);
        }
            
        let target_idx = state.rand_mut().below(cpu_profile::INSTRUCTIONS.len() as u64) as usize;

        let meta = &cpu_profile::INSTRUCTIONS[target_idx];

        let mut inst : u32 = 0;
        let mask: u32 = meta.mask;
        inst |= mask;
        for operand in meta.operands.clone().into_iter() {

            let nb_bits = operand.1-operand.0 + 1;
            let op_max = ((1<<(1-1))-1)^((1<<nb_bits)-1);
            
            let op_value = state.rand_mut().below(op_max) as u32;

            inst |= build_operand(op_value, operand.0, operand.1);
        }

        let mutation = Instruction{
            instruction: inst as u64,
            length: meta.length,
            mask: meta.mask,
            mmatch: meta.mmatch,
            mnemonic: meta.mnemonic.clone(),
            extension: meta.extension.clone(),
            operands: meta.operands.clone(),
        };

        // randomly insert mutant
        // let mut from = state.rand_mut().choose(0..input.instructions.len()) as usize;
        let idx = state.rand_mut().below(input.instructions.len() as u64) as usize;

        if input.instructions[idx].length == mutation.length {
            input.instructions.remove(idx);
            input.instructions.insert(idx, mutation.clone());
        } else {
            // search for for match 
            let mut skipped = true;
            for i in 0..input.instructions.len() {
                if input.instructions[i].length == mutation.length {
                    input.instructions.remove(i);
                    input.instructions.insert(i, mutation.clone());
                    skipped = false;
                    break;        
                } 
            }

            if skipped {
                return Ok(MutationResult::Skipped);
            }
        }

        Ok(MutationResult::Mutated)
    }
}

impl Named for OpcodeMutator {
    fn name(&self) -> &str {
        "OpcodeMutator"
    }
}

impl OpcodeMutator {
    /// Creates a a new [`LoadStoreMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

use libpresifuzz_riscv::cpu_profile::*;

use crate::ISAMutatorsTuple;

#[must_use]
pub fn riscv_mutations<S>() -> impl ISAMutatorsTuple<S>
where
    S: HasRand + HasMetadata + HasMaxSize + HasCorpus,
    // I: HasBytesVec + Clone,
{
    tuple_list!(
        InstDeleteMutator::new(),
        InstDeleteMutator::new(),
        InstDeleteMutator::new(),
        InstDeleteMutator::new(),
        OpcodeMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        OperandMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
        AppendInstMutator::new(),
    )
}

