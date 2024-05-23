// SPDX-FileCopyrightText: 2022 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
use libafl::inputs::Input;
use std::{rc::Rc, string::String, vec::Vec};
use core::{
    cell::RefCell,
    convert::From,
    hash::{BuildHasher, Hasher},
};
use ahash::RandomState;
use libafl_bolts::{Error, HasLen};
use serde::{Deserialize, Serialize};
use libafl::{
    inputs::HasBytesVec,
    mutators::{MutationResult},
};
use libpresifuzz_riscv::instruction::Instruction;
use libpresifuzz_riscv::cpu_profile::INSTRUCTIONS;

// type Instruction = Instruction;

#[derive(Serialize, Deserialize, Clone, Default, PartialEq, Eq, Hash)]
pub struct ISAInput {
    /// The input representation as list of [instruction, offsets, mnemonic]
    instructions: Vec<Instruction>,
}

impl fmt::Debug for ISAInput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut format = String::from("[\n]");
        for (index, &ref insn) in self.instructions.iter().enumerate() {
            format.push_str(&format!("[{}] value:{:x} mnemonic:{} length:{}\n", index, insn.instruction, insn.mnemonic, insn.length));
        }
        format.push_str(&format!("Total length: {}B\n", self.len()));
        format.push_str("]\n");
        return write!(f, "{}", format);
    }
}

impl Input for ISAInput {
    /// Generate a name for this input
    #[must_use]
    fn generate_name(&self, _idx: usize) -> String {
        let mut hasher = RandomState::with_seeds(0, 0, 0, 0).build_hasher();
        for instruction in &self.instructions {
            hasher.write(instruction.instruction.to_be_bytes().as_ref());
        }
        format!("{:016x}", hasher.finish())
    }
}

/// Rc Ref-cell from Input
impl From<ISAInput> for Rc<RefCell<ISAInput>> {
    fn from(input: ISAInput) -> Self {
        Rc::new(RefCell::new(input))
    }
}

impl HasLen for ISAInput {
    #[inline]
    fn len(&self) -> usize {
        let mut len: usize = 0;

        for instruction in &self.instructions {
            len += instruction.length;
        }
        return len; 
    }
}

impl ISAInput {
    /// Creates a new codes input using the given terminals
    #[must_use]
    pub fn new_from_parsed_data(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    #[must_use]
    pub fn from_input<I: HasBytesVec + Clone>(input: &mut I) -> Self {
        let raw_input = input.bytes();
        Self::new(raw_input)
    }

    #[must_use]
    pub fn new(raw_input: &[u8]) -> Self {
        let mut k = 0;
        let mut testcase_insns: Vec<Instruction> = vec![];
        let len = raw_input.len();

        // let's disassemble
        while k+1 < len {

            let mut u16_b: [u8; 2] = [0; 2];
            u16_b.copy_from_slice(&raw_input[k..k+2]);

            let mut insn : u32 = u16::from_le_bytes(u16_b).into();
            let insn_length = if (insn & 0x03) < 0x03 { 2 } else { 4 };  

            match insn_length {
                4 => {
                    if k+1 < len {
                        let mut u16_b: [u8; 2] = [0; 2];
                        u16_b.copy_from_slice(&raw_input[k+2..k+4]);

                        insn |= (u16::from_le_bytes(u16_b) as u32) << 16 as u32;
                    } 
                    k += 4;
                }
                2 => {
                    k += 2;
                    // if (insn>>8)&3 == 3 {
                        // panic!("INVALID DECODING!!!");
                    // }
                }
                _ => {
                    println!("Probably a .2bytes"); 
                }
            };


            for meta in INSTRUCTIONS.iter()  {
                let insn_mask = meta.mask;
                let insn_match = meta.mmatch;
 
                if (insn & insn_match) == insn_mask {

                    let insn_mnemonic = meta.mnemonic.clone();

                    testcase_insns.push(Instruction{
                        instruction: insn as u64, 
                        length: insn_length, 
                        mask: insn_mask, 
                        mmatch: insn_match,
                        mnemonic: insn_mnemonic,
                        extension: meta.extension.clone(),
                        operands: meta.operands.clone(),
                        });
                    break;
                }
            }
        }
        return Self { instructions: testcase_insns };
    }

    /// Create a bytes representation of this input
    pub fn unparse(&self, bytes: &mut Vec<u8>) {
        bytes.clear();
        for instruction in &self.instructions {
            //XXX: consider instruction length to avoid 0 ext
            if instruction.length == 2 {
                let short: u16 = instruction.instruction as u16;
                bytes.extend_from_slice(short.to_le_bytes().as_ref());
            }
            else if instruction.length == 4 {
                let word: u32 = instruction.instruction as u32;
                bytes.extend_from_slice(word.to_le_bytes().as_ref());
            }
            else {
                bytes.extend_from_slice(instruction.instruction.to_le_bytes().as_ref());
            } 
        }
    }

    /// Crop the value to the given length
    pub fn crop(&self, from: usize, to: usize) -> Result<Self, Error> {
        if from < to && to <= self.instructions.len() {
            let mut sub = vec![];
            sub.clone_from_slice(&self.instructions[from..to]);
            Ok(Self { instructions: sub })
        } else {
            Err(Error::illegal_argument("Invalid from or to argument"))
        }
    }
}




//////////////////////////////////////////////////////////
///
///
///
/////////////////////////////////////////////////////////

/// An isa mutator takes input, and mutates it.
/// Simple as that.
// pub trait ISAMutator<S>: Named {
pub trait ISAMutator<S>: Named {
    /// Mutate a given input
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        stage_idx: i32,
    ) -> Result<MutationResult, Error>;

    /// Post-process given the outcome of the execution
    #[inline]
    fn post_exec(
        &mut self,
        _state: &mut S,
        _stage_idx: i32,
        _corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        Ok(())
    }
}

use libafl_bolts::{tuples::HasConstLen, Named};
use libafl::{corpus::CorpusId};
use core::fmt;

/// The index of a mutation in the mutations tuple
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct MutationId(pub(crate) usize);

impl fmt::Display for MutationId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MutationId({})", self.0)
    }
}

impl From<usize> for MutationId {
    fn from(value: usize) -> Self {
        MutationId(value)
    }
}

impl From<u64> for MutationId {
    fn from(value: u64) -> Self {
        MutationId(value as usize)
    }
}

impl From<i32> for MutationId {
    #[allow(clippy::cast_sign_loss)]
    fn from(value: i32) -> Self {
        debug_assert!(value >= 0);
        MutationId(value as usize)
    }
}


/// A `Tuple` of `Mutators` that can execute multiple `Mutators` in a row.
pub trait ISAMutatorsTuple<S>: HasConstLen {
    /// Runs the `mutate` function on all `Mutators` in this `Tuple`.
    fn mutate_all(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        stage_idx: i32,
    ) -> Result<MutationResult, Error>;

    /// Runs the `post_exec` function on all `Mutators` in this `Tuple`.
    fn post_exec_all(
        &mut self,
        state: &mut S,
        stage_idx: i32,
        corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error>;

    /// Gets the [`Mutator`] at the given index and runs the `mutate` function on it.
    fn get_and_mutate(
        &mut self,
        index: MutationId,
        state: &mut S,
        input: &mut ISAInput,
        stage_idx: i32,
    ) -> Result<MutationResult, Error>;

    /// Gets the [`Mutator`] at the given index and runs the `post_exec` function on it.
    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        stage_idx: i32,
        corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error>;

    /// Gets all names of the wrapped [`Mutator`]`s`.
    fn names(&self) -> Vec<&str>;
}

impl<S> ISAMutatorsTuple<S> for () {
    #[inline]
    fn mutate_all(
        &mut self,
        _state: &mut S,
        _input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        Ok(MutationResult::Skipped)
    }

    #[inline]
    fn post_exec_all(
        &mut self,
        _state: &mut S,
        _stage_idx: i32,
        _corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn get_and_mutate(
        &mut self,
        _index: MutationId,
        _state: &mut S,
        _input: &mut ISAInput,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        Ok(MutationResult::Skipped)
    }

    #[inline]
    fn get_and_post_exec(
        &mut self,
        _index: usize,
        _state: &mut S,
        _stage_idx: i32,
        _corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        Ok(())
    }

    #[inline]
    fn names(&self) -> Vec<&str> {
        Vec::new()
    }
}

impl<Head, Tail, S> ISAMutatorsTuple<S> for (Head, Tail)
where
    Head: ISAMutator<S>,
    Tail: ISAMutatorsTuple<S>,
{
    fn mutate_all(
        &mut self,
        state: &mut S,
        input: &mut ISAInput,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        let r = self.0.mutate(state, input, stage_idx)?;
        if self.1.mutate_all(state, input, stage_idx)? == MutationResult::Mutated {
            Ok(MutationResult::Mutated)
        } else {
            Ok(r)
        }
    }

    fn post_exec_all(
        &mut self,
        state: &mut S,
        stage_idx: i32,
        corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        self.0.post_exec(state, stage_idx, corpus_idx)?;
        self.1.post_exec_all(state, stage_idx, corpus_idx)
    }

    fn get_and_mutate(
        &mut self,
        index: MutationId,
        state: &mut S,
        input: &mut ISAInput,
        stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if index.0 == 0 {
            self.0.mutate(state, input, stage_idx)
        } else {
            self.1
                .get_and_mutate((index.0 - 1).into(), state, input, stage_idx)
        }
    }

    fn get_and_post_exec(
        &mut self,
        index: usize,
        state: &mut S,
        stage_idx: i32,
        corpus_idx: Option<CorpusId>,
    ) -> Result<(), Error> {
        if index == 0 {
            self.0.post_exec(state, stage_idx, corpus_idx)
        } else {
            self.1
                .get_and_post_exec(index - 1, state, stage_idx, corpus_idx)
        }
    }

    fn names(&self) -> Vec<&str> {
        let mut ret = self.1.names();
        ret.insert(0, self.0.name());
        ret
    }
}

pub mod riscv_isa;
pub mod scheduled;

