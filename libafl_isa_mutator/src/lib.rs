use libafl_bolts::{
        rands::Rand, Named,
        tuples::{tuple_list, tuple_list_type, NamedTuple},
        HasLen 
};
use libafl::{
    // inputs::BytesInput,
    corpus::Corpus,
    inputs::HasBytesVec,
    mutators::{MutationResult, Mutator},
    random_corpus_id,
    state::{HasCorpus, HasMaxSize, HasRand},
    mutators::{mutations::*, token_mutations::*},
    mutators::MutatorsTuple,
    state::{HasMetadata, StdState},
    Error,
};

use std::collections::HashMap;

mod riscv_inst;
use crate::riscv_inst::Instructions;

pub fn bytes_to_u32(input: &[u8]) -> u32 {
    let mut inst : u32 = 0;

    if input.len() == 2 {
        inst = input[0] as u32 |
            ((input[1] as u32) << 8);
    } else if input.len() == 3 {
        inst = input[0] as u32 |
            ((input[1] as u32) << 8)|
            ((input[2] as u32) << 16);
    } else {
        inst = input[0] as u32 |
            ((input[1] as u32) << 8)|
            ((input[2] as u32) << 16)|
            ((input[3] as u32) << 24);
    }

    return inst;
}

// we assume input is always a multiple of 32bits
//
pub fn quick_disassembling(
    input: &mut Vec<u8>,
) -> Vec<(usize, u32)> {

    // we assume instructions are valid
    // only read first byte to guess instruction size
    let mut ret: Vec<(usize, u32)> = vec![];

    let mut k = 0;
    while (k+3) < input.len() {

        let mut inst: u32 = bytes_to_u32(&input[k..=k+3]) as u32;

            
        let is_32 : bool = if ((inst >> 2) & 7) != 7 && (inst & 3) == 3 { true } else { false };
        let is_16 = ! is_32;

        if is_32 {
            ret.push((k,4));
        } else {
            ret.push((k,2));

            if k>2 {
                ret.push((k-2,2));
            }
        }

        k += 4;
    }

    return ret; 
}

/// Bitflip mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct RISCVMutator<M> 
{
    mutator: M
}

impl<M, I, S> Mutator<I, S> for RISCVMutator<M>
where
    M: Mutator<I, S> + Named,
    S: HasRand,
    I: HasBytesVec + Clone,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {

        let size = input.bytes().len();

        // makes sure our buffer is always a multiple of 32bits
        if size % 4 != 0 {

            let gap = 4 - (size % 4);

            input.bytes_mut().resize(size+gap, 0);
        }

        // println!("\n\n\n\n");
        // println!("Mutator      : {:?}", self.mutator.name());
        // println!("Initial State: {:?}", input.bytes());

        // quick disasembling of input to get the number of instruction and offsets
        // let offsets: Vec<u8> = quick_disassembling(input.bytes_mut());

        // randomly select the offset
        // let idx_even: usize = ((state.rand_mut().below(offsets.len()))) as usize;
        // let idx_even = offsets[idx_even];
                
        let mut mutant = &mut input.clone();

        // extract the content from input
        // let size = if (input.bytes()[idx_even] & 0x3) == 0x3 { 2} else { 4 };
        // let mut inst: u32 = bytes_to_u32(&input.bytes()[idx_even..idx_even+size]) as u32;

        // run the expected mutator
        self.mutator.mutate(state, mutant, _stage_idx);
        
        // println!("sanitization");
        self.sanitize(mutant.bytes_mut());
        // println!("sanitization: ok");

        // let mutant_valid = self.is_valid(mutant.bytes_mut());
//
        // if !mutant_valid {
//
            // return Ok(MutationResult::Skipped);
        // }

        // merge results and return
        // simply replace input by mutant
        let size = input.bytes().len(); 
        input.bytes_mut().drain(0..size);
        input.bytes_mut().resize(mutant.bytes().len(), 0);
        let mut k = 0;
        for p in &mut input.bytes_mut()[0..mutant.bytes().len()] {
            *p = mutant.bytes()[k].clone();
            k += 1;
        }

        if mutant.bytes().len() >= 2 {
            return Ok(MutationResult::Mutated);
        } else {
            return Ok(MutationResult::Skipped);
        }

    }

    // fn mutate_old(
        // &mut self,
        // state: &mut S,
        // input: &mut I,
        // _stage_idx: i32,
    // ) -> Result<MutationResult, Error> {
        // let size = input.bytes().len();
        // println!("Mutator      : {:?}", self.mutator.name());
        // println!("Initial State: {:?}", input.bytes());
       
        // Small inputs should be removed from the corpus 
        // if size < 2 || input.bytes().is_empty() {

            // the current testcase does not contain any valid instruction
            // we want to avoid resource lost, so we generate a new valid one

            // let mut new_inst: u32 = state.rand_mut().below(0xEFFFFFFF) as u32;
            // let (mut valid, mut operand_mask, mut inst_mask, mut mnemonic, mut format, mut priority) = Instructions::is_valid(new_inst);
//
            // while !valid {
//
                // println!("loop for valid a");
                // new_inst = state.rand_mut().below(0xFFFFFFFF) as u32;
                // (valid, operand_mask, inst_mask, mnemonic, format, priority) = Instructions::is_valid(new_inst);
                // if valid {
                    // break;
                // }
            // }

            // set 4bytes length
            // input.bytes_mut().resize(4, 0);
//
            // now copy existing
            // let mut k = 0;
            // for p in &mut input.bytes_mut()[0..4] {
                // *p = (new_inst >> k*8) as u8;
                // k += 1;
            // }
            // println!("Add instruction \t{} \t{} \t{:b}", mnemonic, format, new_inst);
            // println!("{:?}", input.bytes());
//
            // Ok(MutationResult::Mutated)
        // } else {
//
            // let extra =  size - ( size - ( size % 2 ) );
//
            // if extra != 0 {
                // input.bytes_mut().drain(size-extra..size);
            // }
//
            // sanitize buffer
            // let mut k = 0;
            // let mut inst_count = 0;
            // while k < input.bytes().len() {
//
                // println!("loop for valid ab");
//
                // if k+1 < input.bytes().len() {
//
                    // let mut inst: u32 = bytes_to_u32(&input.bytes()[k..=k+1]) as u32;
                    // let (valid, operand_mask, inst_mask, mnemonic, format, priority) = Instructions::is_valid(inst);
                    // if valid && operand_mask < 0xFFFF {
                        // inst_count += 1;
                        // println!("[{}] Buffer contains 16bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                        // k += 2;
                        // continue;
                    // } else if valid {
                        // inst_count += 1;
                        // println!("[{}] Buffer contains 32bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                        // k += 4;
                        // continue;
//
                    // } else {
                        // remove bytes
                        // input.bytes_mut().drain(k..=k+1);
                        // println!("Removing 2bytes at {}: {:?}", k, input.bytes());
                    // }
                // }
            // }
            // let size = input.bytes().len();
//
            // if size == 0 {
                // return Ok(MutationResult::Mutated);
            // }
//
            // instructions are 2-bytes aligned -> even index
            // let idx_even : usize = ((state.rand_mut().below((size-2) as u64) * 2) % (size as u64)) as usize;
            // println!("Index even {}", idx_even);
//
            // let mut inst_size = 0;
//
            // let mut inst: u32 = bytes_to_u32(&input.bytes()[idx_even..=idx_even+1]) as u32;
            // let (valid, operand_mask, inst_mask, mnemonic, format, old_priority) = Instructions::is_valid(inst);
            // if valid == true && operand_mask < 0xFFFF {
//
                // println!("16b inst detected!");
                // inst_size = 2;
            // } else if idx_even+3 < size {
                // if valid == false {
//
                    // println!("Cleaning input from invalid instructions!");
//
                    // clean the input from invalid content and return
                    // input.bytes_mut().drain(idx_even..=(idx_even+3 as usize));
//
                    // return Ok(MutationResult::Mutated);
                // }
//
                // println!("32b inst detected!");
//
                // inst = bytes_to_u32(&input.bytes()[idx_even..=idx_even+3]);
//
                // inst_size = 4;
            // } else {
                // return Ok(MutationResult::Skipped);
            // }
//
            // println!("Match \t{} \t{} \t{:b}", mnemonic, format, inst);
//
            // create a temprary buffer
            // let mut m_bytes = Vec::<u8>::with_capacity(inst_size);
            // if inst_size == 2 {
                // m_bytes.extend((inst as u16).to_le_bytes());
            // } else {
                // m_bytes.extend(inst.to_le_bytes());
            // }
//
            // let mut mutation_valid: bool = false;
//
            // let mut mutant_timeout = 5;
//
            // while mutation_valid == false {
//
                // println!("loop for valid c");
//
                // let mut mutant = &mut input.clone();
                // mutant.bytes_mut().drain(0..size);
                // mutant.bytes_mut().resize(inst_size, 0);
                // let mut k = 0;
                // for p in &mut mutant.bytes_mut()[0..inst_size] {
                    // *p = m_bytes[k].clone();
                    // k += 1;
                // }
//
                // self.mutator.mutate(state, mutant, _stage_idx);
//
                // let mut m_inst : u32 = 0;
//
                // XXX: Mutant could still triggers invalid instruction that would not be detected
                // here as we only check the first 4 bytes
                // match mutant.bytes().len() {
                    // 0 => {
                        // m_inst = 0;
                    // }
                    // 1 => {
                        // m_inst = mutant.bytes()[0] as u32;
                    // }
                    // 2 => {
                        // m_inst = bytes_to_u32(&mutant.bytes());
                    // }
                    // 3 => {
                        // m_inst = bytes_to_u32(&mutant.bytes()[0..3]);
                    // }
                    // _ => {
                        // m_inst = bytes_to_u32(&mutant.bytes()[0..4]);
                    // }
                // }
//
                // let (mutant_valid, mutant_operand_mask, mutant_inst_mask, mutant_mnemonic, mutant_format, mutant_priority) = Instructions::is_valid(m_inst);
//
                // if (mutant_valid && mutant_inst_mask == inst_mask)
                    // || (mutant_valid && mutant_timeout == 1)
                        // || (mutant_valid && old_priority<mutant_priority)
                // {
                    // mutation_valid = mutant_valid;
//
                // }
//
                // if mutant_valid == true {
                    // println!("Match \t{} \t{} \t{:b}", mutant_mnemonic, mutant_format, m_inst);
                    // println!("{:?} copied at {}:{}", mutant.bytes(), idx_even, idx_even+inst_size);
                    // let mut mutant_size = mutant.bytes_mut().len();
//
                    // if mutant.bytes().len() > inst_size {
                        // some bytes were created
                        // input.bytes_mut().splice(idx_even+inst_size..idx_even+inst_size, mutant.bytes()[inst_size..].iter().cloned());
                        // println!("Overflow, copy additional bytes!");
                    // }
//
                    // if mutant.bytes().len() < inst_size {
                        // some bytes were deleted
                        // let off: usize = inst_size - mutant.bytes().len();
                        // input.bytes_mut().drain(idx_even+(inst_size-off)..idx_even+inst_size);
                        // println!("Underflow, delete {}:{} bytes! mutant size is {}", idx_even+(inst_size-off),idx_even+inst_size, mutant_size);
//                        // mutant_size = inst_size - mutant.bytes().len();
                    // }
//
                    // let mut k = 0;
                    // println!("from {} to {}", idx_even,idx_even+mutant_size);
                    // for p in &mut input.bytes_mut()[idx_even..idx_even+mutant_size] {
                        // println!("Replacing {} with {}", *p, mutant.bytes()[k]);
                        // *p = mutant.bytes_mut()[k];
                        // k += 1;
//
                        // if k+1 >= mutant.bytes().len() {
                            // break;
                        // }
                    // }
                    // println!("Mutant: {:?}", input.bytes());
//
                    // break;
                // }
//
                // if mutant_timeout > 0 {
//
                    // mutant_timeout -= 1;
                // } else {
                    // break;
                // }
            // }
//
            // return Ok(MutationResult::Mutated);
        // }
    // }
}

impl<M> Named for RISCVMutator<M> 
{
    fn name(&self) -> &str {
        "RISCVMutator"
    }
}

impl<M> RISCVMutator<M>
{
    /// Creates a new [`RISCVMutator`].
    #[must_use]
    pub fn new(mutator: M) -> Self {
        Self{mutator}
    }
    
    fn is_valid(
        &mut self,
        input: &Vec<u8>,
    ) -> bool {

        let mut valid = true;

        let mut k : usize = 0;
        let mut inst_count = 0;
        while k < input.len() {
            
            if k+1 < input.len() {

                let mut is_16 = false;
                let inst: u32 = bytes_to_u32(&input[k..=k+1]) as u32;
                
                if (inst & 0x3) != 0x3 {

                    let (dis_valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 
                    
                    if dis_valid {
                        // println!("[{}] Buffer contains 16bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                        k += 2;
                        is_16 = true;
                    } else {
                        // println!("[{}] Buffer contains invalid 16bits inst \t{:b}", inst_count, inst);
                        valid = false;
                    }

                }

                if k+3 < input.len() && !is_16{
                
                    let inst: u32 = bytes_to_u32(&input[k..=k+3]) as u32;
                    let (dis_valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 

                    if dis_valid {
                        // println!("[{}] Buffer contains 32bits inst \t{} \t{} \t{:b} \t{:b}", inst_count, mnemonic, format, inst, inst & 0x3);
                        k += 4;
                    } else {
                        // println!("[{}] Buffer contains invalid 32bits inst \t{:b}", inst_count, inst);
                        valid = false;
                        k += 2;
                    }

                } else {
                    k += 2;
                    valid = false;
                }

            } else {
                break;
                valid = false;
            } 
        }

        return valid;
    }

    fn sanitize(
        &mut self,
        input: &mut Vec<u8>,
    ) {

        let size = input.len();

        // let extra =  size - ( size - ( size % 2 ) );
//
        // if extra != 0 {
            // input.drain(size-extra..=size);
        // }
 
        let mut k : usize = 0;
        let mut inst_count = 0;
        while k < input.len() {
            
            if k+1 < input.len() {

                let mut is_16 = false;
                let mut is_32 = false;
                let inst: u32 = bytes_to_u32(&input[k..=k+1]) as u32;
                
                if (inst & 0x3) != 0x3 {

                    // it is a 16bits instruction

                    let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 
                    
                    if valid {
                        // println!("[{}] Buffer contains 16bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                        k += 2;
                        is_16 = true;
                    }
                }

                if k+3 < input.len() && !is_16{
                
                    let inst: u32 = bytes_to_u32(&input[k..=k+3]) as u32;
                    let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 

                    if valid {
                    
                        // println!("[{}] Buffer contains 32bits inst \t{} \t{} \t{:b} \t{:b}", inst_count, mnemonic, format, inst, inst & 0x3);
                        k += 4;
                        is_32 = true;
                    } 
                }
                    
                if !is_32 && !is_16 {
                    
                    input.drain(k..=k+1);
                    k += 2;
                }

            } else {
                break;
            } 
        }

        // sanitize buffer
        // let mut k : usize = 0;
        // let mut inst_count = 0;
        // while k < input.len() {
//
//
            // if k+1 < input.len() {
//
                // let mut inst: u32 = bytes_to_u32(&input[k..=k+1]) as u32;
                // let (valid, operand_mask, inst_mask, mnemonic, format, priority) = Instructions::is_valid(inst);
                // if valid && operand_mask < 0xFFFF {
                    // inst_count += 1;
                    // println!("[{}] Buffer contains 16bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                    // k += 2;
                    // continue;
                // } else if valid {
                    // inst_count += 1;
                    // println!("[{}] Buffer contains 32bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                    // k += 4;
                    // continue;
//
                // } else {
                    // remove bytes
                    // input.drain(k..=k+1);
                    // println!("Removing 2bytes at {}: {:?}", k, input);
                // }
            // }
        // }

    }


}


/// Bitflip2 mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct BitFlip2Mutator;

impl<I, S> Mutator<I, S> for BitFlip2Mutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if input.bytes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let bit_idx = state.rand_mut().choose(0..8);
            
            let bit_1 = 1 << bit_idx;
            let bit_2 = 1 << (bit_idx+1) % 8;

            let byte = state.rand_mut().choose(input.bytes_mut());
            *byte ^= bit_1;
            *byte ^= bit_2;
            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for BitFlip2Mutator {
    fn name(&self) -> &str {
        "BitFlip2Mutator"
    }
}

impl BitFlip2Mutator {
    /// Creates a new [`BitFlip2Mutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Bitflip4 mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct BitFlip4Mutator;

impl<I, S> Mutator<I, S> for BitFlip4Mutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if input.bytes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let bit_idx = state.rand_mut().choose(0..8);
            
            let bit_1 = 1 << bit_idx;
            let bit_2 = 1 << (bit_idx+1) % 8;
            let bit_3 = 1 << (bit_idx+2) % 8;
            let bit_4 = 1 << (bit_idx+3) % 8;

            let byte = state.rand_mut().choose(input.bytes_mut());
            *byte ^= bit_1;
            *byte ^= bit_2;
            *byte ^= bit_3;
            *byte ^= bit_4;
            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for BitFlip4Mutator {
    fn name(&self) -> &str {
        "BitFlip4Mutator"
    }
}

impl BitFlip4Mutator {
    /// Creates a new [`BitFlip4Mutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Byteflip2 mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct ByteFlip2Mutator;

impl<I, S> Mutator<I, S> for ByteFlip2Mutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if input.bytes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let size = input.bytes().len();
            let byte_idx = state.rand_mut().choose(0..size);
       
            let mut payload = input.bytes_mut();
            payload[byte_idx] ^= 0xFF;
            payload[(byte_idx+1) % size] ^= 0xFF;

            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for ByteFlip2Mutator {
    fn name(&self) -> &str {
        "ByteFlip2Mutator"
    }
}

impl ByteFlip2Mutator {
    /// Creates a new [`ByteFlipMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// 2Bytes increment mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct Byte2IncMutator;

impl<I, S> Mutator<I, S> for Byte2IncMutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        let size = input.bytes().len();
        
        if input.bytes().is_empty() || size == 1 {
            Ok(MutationResult::Skipped)
        } else {
            let mut byte_idx = 0;

            if size <= 2 {
                byte_idx = 0;
            } else {
                byte_idx = state.rand_mut().choose(0..size-2);
            }
            // let integer:u16 = a[0] as u16 << 24) | ((slice[i+2] as u32) << 16) | ((slice[i+1] as u32) << 8) | (slice[i] as u32));
            
            let mut a: u16 = input.bytes_mut()[byte_idx] as u16;
            let mut b: u16 = input.bytes_mut()[byte_idx+1] as u16;

            let mut integer: u16 = a << 8 | b;
            integer += 1;

            let a: u8 = (integer & 0xF) as u8;
            let b: u8 = ((integer >> 8) as u8) & 0xF as u8;

            input.bytes_mut()[byte_idx] = a;
            input.bytes_mut()[byte_idx+1] = b;
            
            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for Byte2IncMutator {
    fn name(&self) -> &str {
        "Byte2IncMutator"
    }
}

impl Byte2IncMutator {
    /// Creates a new [`ByteIncMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

/// Byte2 decrement mutation for inputs with a bytes vector
#[derive(Default, Debug)]
pub struct Byte2DecMutator;

impl<I, S> Mutator<I, S> for Byte2DecMutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if input.bytes().is_empty() {
            Ok(MutationResult::Skipped)
        } else {
            let size = input.bytes().len();
            let byte_idx = state.rand_mut().choose(0..size);

            let mut byte_1 = input.bytes_mut()[byte_idx];
            let mut byte_2 = input.bytes_mut()[byte_idx+1];

            let b2_u16 : u16 = byte_2 as u16;
            let mut short: u16 = b2_u16 << 8 & byte_1 as u16;
            short -= 1;

            let lsb = (short & 0xFF00) >> 8; 
            byte_2 = lsb as u8;

            byte_1 = (short & 0xFF) as u8;

            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for Byte2DecMutator {
    fn name(&self) -> &str {
        "Byte2DecMutator"
    }
}

impl Byte2DecMutator {
    /// Creates a a new [`ByteDecMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[derive(Default, Debug)]
pub struct OpcodeMutator;

impl<I, S> Mutator<I, S> for OpcodeMutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
        if input.bytes().is_empty() && input.bytes().len() >= 2 {
            Ok(MutationResult::Skipped)
        } else {

            // randomly generate a valid opcode
            let masks = Instructions::get_masks();
            let opcode_idx = state.rand_mut().choose(0..masks.len());

            // let's replace an existing instruction with the new one
            let offsets = quick_disassembling(input.bytes_mut());

            let old_idx = state.rand_mut().choose(0..offsets.len());    

            let new_inst : u32 = masks[opcode_idx].1;
            use std::mem::transmute;
            let new_inst_bytes: [u8; 4] = unsafe { transmute(new_inst.to_be()) };

            let old_start_pos = offsets[old_idx].0;

            let new_inst_name =  masks[opcode_idx].0;
            // println!("Generating {}", new_inst_name);

            if offsets[old_idx].1 == 2 { 

                input.bytes_mut()[old_start_pos] = new_inst_bytes[1];
                input.bytes_mut()[old_start_pos+1] = new_inst_bytes[0];
                
            } else {
                let size = input.bytes().len(); 

                input.bytes_mut()[old_start_pos] = new_inst_bytes[1];
                input.bytes_mut()[old_start_pos+1] = new_inst_bytes[0];
                if input.bytes().len() < old_start_pos+4 {
                    input.bytes_mut().resize(size+2, 0);
                }
                input.bytes_mut()[old_start_pos+2] = new_inst_bytes[3];
                input.bytes_mut()[old_start_pos+3] = new_inst_bytes[2];
            }

            Ok(MutationResult::Mutated)
        }
    }
}

impl Named for OpcodeMutator {
    fn name(&self) -> &str {
        "OpcodeMutator"
    }
}

impl OpcodeMutator {
    /// Creates a a new [`OpcodeMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
    
}

#[derive(Default, Debug)]
pub struct OperandsMutator;

impl<I, S> Mutator<I, S> for OperandsMutator
where
    S: HasRand,
    I: HasBytesVec,
{
    fn mutate(
        &mut self,
        state: &mut S,
        input: &mut I,
        _stage_idx: i32,
    ) -> Result<MutationResult, Error> {
            // let's replace an existing instruction with the new one
            let offsets = quick_disassembling(input.bytes_mut());

            let inst_idx = state.rand_mut().choose(0..offsets.len());
            let k = offsets[inst_idx].0;

            let is_32 = if offsets[inst_idx].1 == 4 { true } else { false };

            let mut in_copy = input.bytes_mut().to_vec();

            // this is the target_inst that will be mutated
            if k+3 < input.bytes().len() && is_32 {
                let target_inst = &in_copy[k..=k+3];
                let mut target_inst_u32: u32 = bytes_to_u32(&target_inst[0..=3]) as u32;
                                                
                let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(target_inst_u32); 
                if mask_arr.len() == 0{
                    return Ok(MutationResult::Skipped);
                }

                let max_bits_to_flip = state.rand_mut().choose(0..mask_arr.len());

                for i in 0..max_bits_to_flip {
                
                    let bit_to_flip = state.rand_mut().choose(0..mask_arr.len());
                    let bit_to_flip = mask_arr[bit_to_flip];

                    target_inst_u32 = target_inst_u32 ^ (1<<bit_to_flip);
                }

                input.bytes_mut()[k] = ((target_inst_u32) & 0xFF) as u8;
                input.bytes_mut()[k+1] = ((target_inst_u32 >> 8)  & 0xFF) as u8;
                input.bytes_mut()[k+2] = ((target_inst_u32 >> 16) & 0xFF) as u8;
                input.bytes_mut()[k+3] = ((target_inst_u32 >> 24) & 0xFF) as u8;

            } else {
                let target_inst = &in_copy[k..=k+1];
                let mut target_inst_u32: u32 = bytes_to_u32(&target_inst[0..=1]) as u32;
                                                
                let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(target_inst_u32); 

                if mask_arr.len() == 0{
                    return Ok(MutationResult::Skipped);
                }

                let max_bits_to_flip = state.rand_mut().choose(0..mask_arr.len());

                for i in 0..max_bits_to_flip {
                
                    let bit_to_flip = state.rand_mut().choose(0..mask_arr.len());
                    let bit_to_flip = mask_arr[bit_to_flip];

                    target_inst_u32 = target_inst_u32 ^ (1<<bit_to_flip);
                }
            }
                    
            Ok(MutationResult::Mutated)
    }
}

impl Named for OperandsMutator {
    fn name(&self) -> &str {
        "OpcodeMutator"
    }
}

impl OperandsMutator {
    /// Creates a a new [`OperandsMutator`].
    #[must_use]
    pub fn new() -> Self {
        Self
    }
    
}

/// Tuple type of the mutations that compose the Havoc mutator
pub type RISCVMutationsType<M> = tuple_list_type!(
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    RISCVMutator<M>,
    // RISCVMutator<M>,
    // RISCVMutator<M>,
    // RISCVMutator<M>,
    // RISCVMutator<M>,
    // RISCVMutator<M>,
    // RISCVOpcodeMutator,
    // BitFlipMutator,
    // BitFlip2Mutator,
    // BitFlip4Mutator,
    // ByteFlipMutator,
    // ByteFlip2Mutator,
    // ByteIncMutator,
    // ByteDecMutator,
    // Byte2IncMutator,
    // Byte2DecMutator,
    // Byte4IncMutator,
    // Byte4DecMutator,
    // ByteRandMutator,
    // DeleteInstructionMutator,
    // CloneInstructionMutator,
    // RISCVOpcodeMutator,

    // BitFlipMutator,
    // ByteFlipMutator,
    // ByteIncMutator,
    // ByteDecMutator,
    // ByteNegMutator,
    // ByteRandMutator,
    // ByteAddMutator,
    // WordAddMutator,
    // DwordAddMutator,
    // QwordAddMutator,
    // ByteInterestingMutator,
    // WordInterestingMutator,
    // DwordInterestingMutator,
    // BytesDeleteMutator,
    // BytesDeleteMutator,
    // BytesDeleteMutator,
    // BytesDeleteMutator,
    // BytesExpandMutator,
    // BytesInsertMutator,
    // BytesRandInsertMutator,
    // BytesSetMutator,
    // BytesRandSetMutator,
    // BytesCopyMutator,
    // BytesInsertCopyMutator,
    // BytesSwapMutator,
    // CrossoverInsertMutator,
    // CrossoverReplaceMutator,
);

/// Get the mutations that compose the Havoc mutator
#[must_use]
pub fn riscv_mutations<I, S>() -> impl MutatorsTuple<I, S>
where
    S: HasRand + HasMetadata + HasMaxSize,
    I: HasBytesVec + Clone,
    // M: Mutator<I, S> + Named,
{
    tuple_list!(
        RISCVMutator::new(BytesExpandMutator::new()),
        RISCVMutator::new(BytesInsertMutator::new()),
        RISCVMutator::new(BytesRandInsertMutator::new()),
        // RISCVMutator::new(BytesSetMutator::new()),
        // RISCVMutator::new(BytesRandSetMutator::new()),
        RISCVMutator::new(BytesCopyMutator::new()),
        RISCVMutator::new(BytesInsertCopyMutator::new()),
        // RISCVMutator::new(BytesSwapMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OpcodeMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),
        RISCVMutator::new(OperandsMutator::new()),


        // RISCVMutator::new(BitFlipMutator::new()),
        // RISCVMutator::new(BitFlip2Mutator::new()),
        // RISCVMutator::new(BitFlip4Mutator::new()),
        // RISCVMutator::new(ByteFlipMutator::new()),
        // RISCVMutator::new(ByteNegMutator::new()),
        // RISCVMutator::new(ByteAddMutator::new()),
        // RISCVMutator::new(WordAddMutator::new()),
        // RISCVMutator::new(DwordAddMutator::new()),
        // RISCVMutator::new(QwordAddMutator::new()),
        // RISCVMutator::new(ByteInterestingMutator::new()),
        // RISCVMutator::new(WordInterestingMutator::new()),
        // RISCVMutator::new(DwordInterestingMutator::new()),
        // RISCVMutator::new(BytesDeleteMutator::new()),
        // RISCVMutator::new(BytesDeleteMutator::new()),
        // RISCVMutator::new(BytesDeleteMutator::new()),
        // RISCVMutator::new(BytesDeleteMutator::new()),
        // RISCVMutator::new(BytesExpandMutator::new()),
        // RISCVMutator::new(BytesInsertMutator::new()),
        // RISCVMutator::new(BytesRandInsertMutator::new()),
        // RISCVMutator::new(BytesSetMutator::new()),
        // RISCVMutator::new(BytesRandSetMutator::new()),
        // RISCVMutator::new(BytesCopyMutator::new()),
        // RISCVMutator::new(BytesInsertCopyMutator::new()),
        // RISCVMutator::new(BytesSwapMutator::new()),
        // RISCVMutator::new(OpcodeMutator::new()),
        // RISCVMutator::new(OperandsMutator::new()),

        // RISCVOpcodeMutator::new(),
        // BitFlipMutator::new(),
        // BitFlip2Mutator::new(),
        // BitFlip4Mutator::new(),
        // ByteFlipMutator::new(),
        // ByteFlip2Mutator::new(),
        // ByteIncMutator::new(),
        // ByteDecMutator::new(),
        // Byte2IncMutator::new(),
        // Byte2DecMutator::new(),
        // Byte4IncMutator::new(),
        // Byte4DecMutator::new(),
        // ByteRandMutator::new(),
        // DeleteInstructionMutator::new(),
        // CloneInstructionMutator::new(),
        // RISCVOpcodeMutator::new(),

        // ByteNegMutator::new(),
        // ByteAddMutator::new(),
        // WordAddMutator::new(),
        // DwordAddMutator::new(),
        // QwordAddMutator::new(),
        // ByteInterestingMutator::new(),
        // WordInterestingMutator::new(),
        // DwordInterestingMutator::new(),
        // BytesDeleteMutator::new(),
        // BytesDeleteMutator::new(),
        // BytesDeleteMutator::new(),
        // BytesDeleteMutator::new(),
        // BytesExpandMutator::new(),
        // BytesInsertMutator::new(),
        // BytesRandInsertMutator::new(),
        // BytesSetMutator::new(),
        // BytesRandSetMutator::new(),
        // BytesCopyMutator::new(),
        // BytesInsertCopyMutator::new(),
        // BytesSwapMutator::new(),
        // CrossoverInsertMutator::new(),
        // CrossoverReplaceMutator::new(),
    )
}

#[cfg(feature = "std")]
#[cfg(test)]
mod tests {

    use super::*;
    use libafl::{
        bolts::{rands::StdRand, tuples::tuple_list},
        corpus::{Corpus, InMemoryCorpus, Testcase},
        events::NopEventManager,
        executors::{ExitKind, InProcessExecutor},
        feedbacks::ConstFeedback,
        fuzzer::Fuzzer,
        inputs::BytesInput,
        monitors::SimpleMonitor,
        mutators::{mutations::BitFlipMutator, StdScheduledMutator},
        schedulers::RandScheduler,
        stages::StdMutationalStage,
        state::{HasCorpus, StdState},
        StdFuzzer,
    };
    use libafl::prelude::HasConstLen;
    use libafl::prelude::current_time;

    fn test_mutations<I, S>() -> impl MutatorsTuple<I, S>
    where
        S: HasRand + HasMetadata + HasMaxSize,
        I: HasBytesVec + Clone,
    {
        tuple_list!(
            // RISCVMutator::new(BitFlipMutator::new()),
            // RISCVMutator::new(BitFlip2Mutator::new()),
            // RISCVMutator::new(BitFlip4Mutator::new()),
            // RISCVMutator::new(ByteFlipMutator::new()),
            // RISCVMutator::new(ByteNegMutator::new()),
            // RISCVMutator::new(ByteAddMutator::new()),
            // RISCVMutator::new(WordAddMutator::new()),
            // RISCVMutator::new(DwordAddMutator::new()),
            // RISCVMutator::new(QwordAddMutator::new()),
            // RISCVMutator::new(ByteInterestingMutator::new()),
            // RISCVMutator::new(WordInterestingMutator::new()),
            // RISCVMutator::new(DwordInterestingMutator::new()),
            // RISCVMutator::new(BytesDeleteMutator::new()),
            // RISCVMutator::new(BytesDeleteMutator::new()),
            // RISCVMutator::new(BytesDeleteMutator::new()),
            // RISCVMutator::new(BytesDeleteMutator::new()),
            RISCVMutator::new(BytesExpandMutator::new()),
            RISCVMutator::new(BytesInsertMutator::new()),
            RISCVMutator::new(BytesRandInsertMutator::new()),
            // RISCVMutator::new(BytesSetMutator::new()),
            // RISCVMutator::new(BytesRandSetMutator::new()),
            RISCVMutator::new(BytesCopyMutator::new()),
            RISCVMutator::new(BytesInsertCopyMutator::new()),
            // RISCVMutator::new(BytesSwapMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OperandsMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            RISCVMutator::new(OpcodeMutator::new()),
            // RISCVMutator::new(CrossoverInsertMutator::new()),
            // RISCVMutator::new(CrossoverReplaceMutator::new()),



            // RISCVMutator::new(ByteFlip2Mutator::new()),
            // RISCVMutator::new(ByteIncMutator::new()),
            // RISCVMutator::new(ByteDecMutator::new()),
            // RISCVMutator::new(Byte2IncMutator::new()),
            // Byte2DecMutator::new(),
            // Byte4IncMutator::new(),
            // Byte4DecMutator::new(),
            // ByteRandMutator::new(),
            // DeleteInstructionMutator::new(),
            // CloneInstructionMutator::new(),
            // RISCVOpcodeMutator::new(),
        )
    }
    
    // #[test]
    // fn test_fuzz_mutator() {
        // let rand = StdRand::with_seed(0);
//
        // let mut corpus = InMemoryCorpus::<BytesInput>::new();
        // let testcase = Testcase::new(vec![0; 4].into());
        // corpus.add(testcase).unwrap();
        // corpus.add(Testcase::new(vec![0x14,0xec,0x07,0x00,0x20,0x0b,0x00,0x00,0x00,0x00,0x59,0x59,0x59,0x59,0x20,0x00,0x59,0x58,0x7b,0x01,0x00,0x7f,0x59,0x4f,0x54,0x80,0x54,0x54,0x54,0x54,0x54,0x54,0x63,0x00,0x00,0x00,0x00,0x10,0x00,0x20,0x93,0x00,0x00,0xfa,0x00,0x93,0xfa,0x00,0x93,0x00,0x00,0x20,0x00,0x93,0x93,0x93,0x00,0x07,0x71,0x00,0x00,0x00,0x00,0xfa,0x06,0x00,0x01,0x93])).unwrap();
        // corpus.add(Testcase::new(vec![0x0, 0x0, 0x10, 0x0].into())).unwrap();
        // corpus.add(Testcase::new(vec![0x00, 0x20, 0x58, 0x59, 0x4f, 0x59, 0x7f, 0x00].into())).unwrap();
        // corpus.add(Testcase::new(vec![0x00].into())).unwrap();
        // corpus.add(Testcase::new(vec![0x00, 0x20, 0xFF, 0x59, 0x4f, 0x59, 0x7f, 0x00].into())).unwrap();
        // corpus.add(Testcase::new(vec![0, 32, 255, 89, 79, 89, 127, 0].into())).unwrap();
        // corpus.add(Testcase::new(vec![0xAB, 0xAB, 0xAB, 0xAB ,0x00, 0x20, 0x58, 0x59, 0x4f, 0x59, 0x7f, 0x00].into())).unwrap();
        // corpus.add(Testcase::new(vec![0x00, 0x00, 0x20, 0x13].into())).unwrap();

        // let mut feedback = ConstFeedback::new(false);
        // let mut objective = ConstFeedback::new(false);
//
        // let mut state = StdState::new(
            // rand,
            // corpus,
            // InMemoryCorpus::<BytesInput>::new(),
            // &mut feedback,
            // &mut objective,
        // )
        // .unwrap();
//
        // let _monitor = SimpleMonitor::new(|s| {
            // println!("{s}");
        // });
        // let mut event_manager = NopEventManager::new();
//
        // let feedback = ConstFeedback::new(false);
        // let objective = ConstFeedback::new(false);
//
        // let scheduler = RandScheduler::new();
        // let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);
//
        // let mut harness = |_buf: &BytesInput| ExitKind::Ok;
        // let mut executor = InProcessExecutor::new(
            // &mut harness,
            // tuple_list!(),
            // &mut fuzzer,
            // &mut state,
            // &mut event_manager,
        // )
        // .unwrap();
//
        // let mutator = StdScheduledMutator::new(test_mutations());
        // let mut stages = tuple_list!(StdMutationalStage::new(mutator));
//
        // for i in 0..10000 {
            // fuzzer
                // .fuzz_one(&mut stages, &mut executor, &mut state, &mut event_manager)
                // .unwrap_or_else(|_| panic!("Error in iter {i}"));
        // }

        // let state_serialized = postcard::to_allocvec(&state).unwrap();
        // let state_deserialized: StdState<
            // _,
            // InMemoryCorpus<BytesInput>,
            // StdRand,
            // InMemoryCorpus<BytesInput>,
        // > = postcard::from_bytes(state_serialized.as_slice()).unwrap();
        // assert_eq!(state.corpus().count(), state_deserialized.corpus().count());
//
        // let corpus_serialized = postcard::to_allocvec(state.corpus()).unwrap();
        // let corpus_deserialized: InMemoryCorpus<BytesInput> =
            // postcard::from_bytes(corpus_serialized.as_slice()).unwrap();
        // assert_eq!(state.corpus().count(), corpus_deserialized.count());
//

    // }

    #[test]
    fn test_mutator() {

        let mut inputs = vec![
            BytesInput::new(vec![0x14,0xec,0x07,0x00,0x20,0x0b,0x00,0x00,0x00,0x00,0x59,0x59,0x59,0x59,0x20,0x00,0x59,0x58,0x7b,0x01,0x00,0x7f,0x59,0x4f,0x54,0x80,0x54,0x54,0x54,0x54,0x54,0x54,0x63,0x00,0x00,0x00,0x00,0x10,0x00,0x20,0x93,0x00,0x00,0xfa,0x00,0x93,0xfa,0x00,0x93,0x00,0x00,0x20,0x00,0x93,0x93,0x93,0x00,0x07,0x71,0x00,0x00,0x00,0x00,0xfa,0x06,0x00,0x01,0x93]),
            BytesInput::new(vec![0x0, 0x0, 0x10, 0x0]),
            BytesInput::new(vec![0x00, 0x20, 0x58, 0x59, 0x4f, 0x59, 0x7f, 0x00]),
            BytesInput::new(vec![0x00]),
            BytesInput::new(vec![0x00, 0x20, 0xFF, 0x59, 0x4f, 0x59, 0x7f, 0x00]),
            BytesInput::new(vec![0, 32, 255, 89, 79, 89, 127, 0]),
            BytesInput::new(vec![0xAB, 0xAB, 0xAB, 0xAB ,0x00, 0x20, 0x58, 0x59, 0x4f, 0x59, 0x7f, 0x00]),
            BytesInput::new(vec![0x00, 0x00, 0x20, 0x13]),
        ];

        let rand = StdRand::with_seed(current_time().as_nanos() as u64);
        let mut corpus = InMemoryCorpus::<BytesInput>::new();
        let testcase = Testcase::new(vec![0; 4].into());
        corpus.add(testcase).unwrap();

        let mut feedback = ConstFeedback::new(false);
        let mut objective = ConstFeedback::new(false);

        let mut state = StdState::new(
            rand,
            corpus,
            InMemoryCorpus::<BytesInput>::new(),
            &mut feedback,
            &mut objective,
        )
        .unwrap();

        let mutators_names = vec![
            // "BitFlipMutator",
            // "BitFlip2Mutator",
            // "BitFlip4Mutator",
            // "ByteFlipMutator",
            // "ByteNegMutator",
            // "ByteAddMutator",
            // "WordAddMutator",
            // "DwordAddMutator",
            // "QwordAddMutator",
            // "ByteInterestingMutator",
            // "WordInterestingMutator",
            // "DwordInterestingMutator",
            // "BytesDeleteMutator",
            // "BytesDeleteMutator",
            // "BytesDeleteMutator",
            // "BytesDeleteMutator",
            "BytesExpandMutator",
            "BytesInsertMutator",
            "BytesRandInsertMutator",
            // "BytesSetMutator",
            // "BytesRandSetMutator",
            "BytesCopyMutator",
            "BytesInsertCopyMutator",
            // "BytesSwapMutator",
            "OpcodeMutator",
            "OperandsMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
            "OpcodeMutator",
        ];
        
        let mut inst_stats = HashMap::new();

        let mut stats = HashMap::from([
            // ("BitFlipMutator", 0),
            // ("BitFlip2Mutator", 0),
            // ("BitFlip4Mutator", 0),
            // ("ByteFlipMutator", 0),
            // ("ByteNegMutator", 0),
            // ("ByteAddMutator", 0),
            // ("WordAddMutator", 0),
            // ("DwordAddMutator", 0),
            // ("QwordAddMutator", 0),
            // ("ByteInterestingMutator", 0),
            // ("WordInterestingMutator", 0),
            // ("DwordInterestingMutator", 0),
            // ("BytesDeleteMutator", 0),
            // ("BytesDeleteMutator", 0),
            // ("BytesDeleteMutator", 0),
            // ("BytesDeleteMutator", 0),
            ("BytesExpandMutator", 0),
            ("BytesInsertMutator", 0),
            ("BytesRandInsertMutator", 0),
            // ("BytesSetMutator", 0),
            // ("BytesRandSetMutator", 0),
            ("BytesCopyMutator", 0),
            ("BytesInsertCopyMutator", 0),
            // ("BytesSwapMutator", 0),
            ("OpcodeMutator", 0),
            ("OperandsMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
            ("OpcodeMutator", 0),
        ]); 

        let mut mutations = test_mutations();
        for _ in 0..100 {
            let mut new_testcases: Vec<BytesInput> = Vec::<BytesInput>::new();
            for input in &mut inputs {
                for idx in 0..(mutations.len()) {
                    let mut mutant = input.clone();
                    match mutations
                        .get_and_mutate(idx.into(), &mut state, &mut mutant, 0)
                        .unwrap()
                    {
                        MutationResult::Mutated => {
                            match idx {
                                _ => {
                                    new_testcases.push(mutant.clone());
                                    // println!("Mutant: {:?}", mutant.clone().bytes());
                                    // println!("-> ok");

                                    let mutation_name = mutators_names[idx];

                                    stats.entry(mutation_name).and_modify(|counter| *counter += 1).or_insert(1); 

                                    let input = mutant.bytes();
                                    let mut k : usize = 0;
                                    let mut inst_count = 0;
                                    while k < input.len() {
                                        
                                        if k+1 < input.len() {

                                            let mut is_16 = false;
                                            let mut is_32 = false;
                                            let inst: u32 = bytes_to_u32(&input[k..=k+1]) as u32;
                                            
                                            if (inst & 0x3) != 0x3 {

                                                // it is a 16bits instruction

                                                let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 
                                                
                                                if valid {
                                                    // println!("[{}] Buffer contains 16bits inst \t{} \t{} \t{:b}", inst_count, mnemonic, format, inst);
                                                    k += 2;
                                                    inst_stats.entry(mnemonic).and_modify(|counter| *counter += 1).or_insert(1); 
                                                    is_16 = true;
                                                } 
                                            }

                                            if k+3 < input.len() && !is_16{
                                            
                                                let inst: u32 = bytes_to_u32(&input[k..=k+3]) as u32;
                                                let (valid, operand_mask, inst_mask, mnemonic, format, priority, mask_arr) = Instructions::is_valid(inst); 

                                                if valid {
                                                    // println!("[{}] Buffer contains 32bits inst \t{} \t{} \t{:b} \t{:b}", inst_count, mnemonic, format, inst, inst & 0x3);
                                                    k += 4;
                                                    inst_stats.entry(mnemonic).and_modify(|counter| *counter += 1).or_insert(1); 
                                                    is_32 = true;
                                                }

                                            } 

                                            if !is_32 && !is_16 {
                                                // println!("[{}] Buffer contains invalid 16bits inst \t{:b}", inst_count, inst);
                                                k += 2;
                                            }

                                        } else {
                                            break;
                                        } 
                                    }

                                    println!("{:?}", stats);
                                    println!("{:?} {}", inst_stats, inst_stats.len());
                                }
                            }
                        }
                        MutationResult::Skipped => {
                            println!("-> discarded");
                            // stats.entry("Discarded").and_modify(|counter| *counter += 1).or_insert(1);
                            ()
                        },
                    };

                }
            }
            for new_mutant in &new_testcases {
                inputs.push(new_mutant.clone());
            }
        }
    }
}

