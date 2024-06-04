// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

pub mod spike_dasm;
pub mod objdump_dasm;
pub mod helper;
pub mod gen;

use std::ops::{Deref, DerefMut};

use libafl::Error;
use libafl_bolts::ErrorBacktrace;

use self::gen::{gen_branch_instruction, gen_jal_instruction, gen_jalr_instruction, gen_cj_instruction, gen_cjr_instruction, gen_cjalr_instruction, gen_cbeqz_instruction, gen_cbnez_instruction};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum InstructionType { 
    Normal,

    CondBranchRelative((u8, i32)), // reg, relative offset (todo: store relative address?)
    CondBranchRelativeCMP((u8, u8, i32)), // reg1, reg2, offset (todo: store relative address?)
    BranchRelativeStore((u8, i32)), // reg, relative address (e.g. jal)
    BranchAbsoluteStore((u8, u8, i32)), // reg, reg, offset

}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct DasmInstruction {
    pub mnemonic: String,
    pub args: Option<String>,
    pub bytes: u64,
    pub size: u8, // instruction size (2 or 4 bytes)

    pub ins_type: InstructionType,
}

impl DasmInstruction {
    pub fn from_le(disas: &mut Box<dyn Dasm>, value: u32, address: u64) -> Result<Self, Error> {
        disas.process_single(&RiscvInstruction::from_be32(value.swap_bytes()), address)
    }

    pub fn from_be(disas: &mut Box<dyn Dasm>, value: u32, address: u64) -> Result<Self, Error> {
        disas.process_single(&RiscvInstruction::from_be32(value), address)
    }

    pub fn from_le_bytes(disas: &mut Box<dyn Dasm>, bytes: [u8; 4], address: u64) -> Result<Self, Error> {
        let value = u32::from_le_bytes(bytes);
        disas.process_single(&RiscvInstruction::from_be32(value), address)
    }

    pub fn from_be_bytes(disas: &mut Box<dyn Dasm>, bytes: [u8; 4], address: u64) -> Result<Self, Error> {
        let value = u32::from_be_bytes(bytes);
        disas.process_single(&RiscvInstruction::from_be32(value), address)
    }

    pub fn new_jal(disas: &mut Box<dyn Dasm>, address: u64, reg: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        let (v, _) = gen_jal_instruction(reg, offset, safe)?;
        disas.process_single(&RiscvInstruction::from_be32(v), address)
    }

    pub fn new_cj(disas: &mut Box<dyn Dasm>, address: u64, offset: i16, safe: bool) -> Result<Self, Error> {
        let (v, _) = gen_cj_instruction(offset, safe)?;
        disas.process_single(&RiscvInstruction::from_be32(v as u32), address)
    }

    pub fn new_cjr(disas: &mut Box<dyn Dasm>, address: u64, reg: u8) -> Result<Self, Error> {
        let (v, _) = gen_cjr_instruction(reg)?;
        disas.process_single(&RiscvInstruction::from_be32(v as u32), address)
    }

    pub fn new_cjalr(disas: &mut Box<dyn Dasm>, address: u64, reg: u8) -> Result<Self, Error> {
        let (v, _) = gen_cjalr_instruction(reg)?;
        disas.process_single(&RiscvInstruction::from_be32(v as u32), address)
    }

    pub fn new_jalr(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32) -> Result<Self, Error> {
        let (v, _) = gen_jalr_instruction(reg1, reg2, offset)?;
        disas.process_single(&RiscvInstruction::from_be32(v), address)
    }

    pub fn new_cbeqz(disas: &mut Box<dyn Dasm>, address: u64, reg: u8, offset: i16, safe: bool) -> Result<Self, Error> {
        let (v, _) = gen_cbeqz_instruction(reg, offset, safe)?;
        disas.process_single(&RiscvInstruction::from_be32(v as u32), address)
    }

    pub fn new_cbnez(disas: &mut Box<dyn Dasm>, address: u64, reg: u8, offset: i16, safe: bool) -> Result<Self, Error> {
        let (v, _) = gen_cbnez_instruction(reg, offset, safe)?;
        disas.process_single(&RiscvInstruction::from_be32(v as u32), address)
    }

    fn new_branch_instruction(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, btype: u8, safe: bool) -> Result<Self, Error> {
        let (v, _) = gen_branch_instruction(reg1, reg2, offset, btype, safe)?;
        disas.process_single(&RiscvInstruction::from_be32(v), address)
    }

    pub fn new_beq(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 0, safe)
    }

    pub fn new_bne(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 1, safe)
    }

    pub fn new_blt(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 4, safe)
    }

    pub fn new_bge(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 5, safe)
    }

    pub fn new_bltu(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 6, safe)
    }

    pub fn new_bgeu(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, reg2: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, reg2, offset, 7, safe)
    }

    /* x0 pseudo instructions */
    pub fn new_beqz(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, 0, offset, 0, safe)
    }

    /* x0 pseudo instructions */
    pub fn new_bnez(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, 0, offset, 1, safe)
    }

    /* x0 pseudo instructions */
    pub fn new_bltz(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, 0, offset, 4, safe)
    }

    /* x0 pseudo instructions */
    pub fn new_bgez(disas: &mut Box<dyn Dasm>, address: u64, reg1: u8, offset: i32, safe: bool) -> Result<Self, Error> {
        Self::new_branch_instruction(disas, address, reg1, 0, offset, 5, safe)
    }
    //pub fn new_relative_branch()
}

pub trait Dasm : std::fmt::Debug {
    fn process_single(&mut self, ins: &RiscvInstruction, address: u64)  -> Result<DasmInstruction, Error>;
    fn process_slice(&mut self, ins: &RiscvInstructions, address: u64) -> Result<Vec<(u64, DasmInstruction)>, Error>;
}



#[derive(Debug, PartialEq, Clone)]
pub enum RiscvInstruction {
    U16(u16),
    U32(u32),
}

impl RiscvInstruction {

    pub fn be_value(&self) -> u32 {
        match self {
            RiscvInstruction::U16(x) => (*x).swap_bytes() as u32, 
            RiscvInstruction::U32(x) => (*x).swap_bytes(), 
        }
    }

    pub fn from_be16(value: u16) -> Result<RiscvInstruction, Error> {
        if !(value&3 != 3) { 
            return Err(Error::Unknown(format!("value does not match length encoding (val: {:x})", value).to_string(), ErrorBacktrace::new()));
        }
        Ok(RiscvInstruction::U16(value.swap_bytes()))
    }

    pub fn from_be32(value: u32) -> RiscvInstruction {
        if value&3 != 3 {
            RiscvInstruction::U16((value as u16).swap_bytes())
        }
        else {
            RiscvInstruction::U32(value.swap_bytes())
        }
    }
}

/* vector of either u16 or u32 sized riscv instructions (instructions are little endian encoded) */
#[derive(Debug, PartialEq, Clone)]
pub struct RiscvInstructions {
    buffer: Vec<RiscvInstruction>,
    len: usize,
}

impl Deref for RiscvInstructions {
    type Target = Vec<RiscvInstruction>;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl DerefMut for RiscvInstructions {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer
    }
}

impl RiscvInstructions {

    pub fn sanity_check(&self) -> Result<(), Error> {

        for (idx, e) in self.buffer.iter().enumerate() {
            println!("{:?}", e);
            match e {
                RiscvInstruction::U16(x) => {
                    if !((x>>8)&3 != 3) {
                        return Err(Error::Unknown(format!("2 byte opcode at offset {} does not match length encoding (val: {:x})", idx, x.to_be()).to_string(), ErrorBacktrace::new()));
                    }
                }
                RiscvInstruction::U32(x) => {
                    if !((x>>24)&3 == 3) {
                        return Err(Error::Unknown(format!("4 byte opcode at offset {} does not match length encoding (val: {:x})", idx, x.to_be()).to_string(), ErrorBacktrace::new()));
                    }
                }
            }
        }

        return Ok(());
    }

    pub fn new() -> RiscvInstructions {
        RiscvInstructions{buffer: Vec::<RiscvInstruction>::new(), len: 0}
    }

    pub fn push(&mut self, ins: RiscvInstruction) {
        self.len += match &ins {
            RiscvInstruction::U16(_) => 2,
            RiscvInstruction::U32(_) => 4,
        };
        self.buffer.push(ins);
    }

    pub fn from_be(input: Vec<u8>) -> Result<RiscvInstructions, Error> {

        let mut buffer = Vec::<RiscvInstruction>::new();
        let mut len = 0;

        let mut k = 0;

        while k < input.len() {
            if k+3 < input.len() {
                if input[k+3]&3 == 3 {
                    let value_bytes: [u8; 4] = [input[k+0], input[k+1], input[k+2], input[k+3]];
                    let value = u32::from_le_bytes(value_bytes);
                    buffer.push(RiscvInstruction::U32(value));
                    len += 4; 
                    k+=4;
                }
                else {
                    let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                    let value = u16::from_le_bytes(value_bytes);
                    buffer.push(RiscvInstruction::U16(value));
                    len += 2;
                    k+=2;
                }
            }
            else if k+1 < input.len() {
                let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                let value = u16::from_le_bytes(value_bytes);
                buffer.push(RiscvInstruction::U16(value));
                len += 2; 
                k+=2;
            }
            else {
                break;
            }
        }

        let ret = RiscvInstructions{buffer, len};
        let r = ret.sanity_check();

        match r {
            Ok(_) => Ok(ret),
            Err(x) => Err(x),
        }
    }

    pub fn from_be_lossy(input: Vec<u8>) -> RiscvInstructions {
        let mut buffer = Vec::<RiscvInstruction>::new();
        let mut len = 0;

        let mut k = 0;

        while k < input.len() {
            if k+3 < input.len() {
                if input[k+3]&3 == 3 {
                    let value_bytes: [u8; 4] = [input[k+0], input[k+1], input[k+2], input[k+3]];
                    let value = u32::from_le_bytes(value_bytes);
                    buffer.push(RiscvInstruction::U32(value));
                    len += 4; 
                    k+=4;

                    assert!(((value>>24)&3 == 3));

                }
                else {
                    let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                    let value = u16::from_le_bytes(value_bytes);


                    if !((value>>8)&3 != 3) {
                        let value_bytes: [u8; 4] = [input[k+2], input[k+3], input[k+0], input[k+1]];
                        let value = u32::from_le_bytes(value_bytes);    
                        buffer.push(RiscvInstruction::U32(value));
                        len += 4; 
                        k+=4;
                    }
                    else {
                        buffer.push(RiscvInstruction::U16(value));
                        len += 2; 
                        k+=2;
                    }

                }
            }
            else if k+1 < input.len() {
                let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                let value = u16::from_le_bytes(value_bytes);
                k+=2;

                /* only add the last element if it matches with the len encoding; otherwise skip the last 2 bytes */
                if !((value>>8)&3 == 3) {
                    buffer.push(RiscvInstruction::U16(value));
                    len += 2; 
                }
            }
            else {
                break;
            }
        }

        RiscvInstructions{buffer, len} 
    }

    pub fn from_le(input: Vec<u8>) -> RiscvInstructions {
        let mut buffer = Vec::<RiscvInstruction>::new();
        let mut len = 0;
        let mut k = 0;

        while k < input.len() {
            if k+3 < input.len() {
                if input[k]&3 == 3 {
                    let value_bytes: [u8; 4] = [input[k+0], input[k+1], input[k+2], input[k+3]];
                    let value = u32::from_be_bytes(value_bytes);
                    buffer.push(RiscvInstruction::U32(value));
                    len += 4; 
                    k+=4;
                }
                else {
                    let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                    let value = u16::from_be_bytes(value_bytes);
                    buffer.push(RiscvInstruction::U16(value));
                    len += 2; 
                    k+=2;

                }
            }
            else if k+1 < input.len() {
                let value_bytes: [u8; 2] = [input[k+0], input[k+1]];
                let value = u16::from_be_bytes(value_bytes);
                buffer.push(RiscvInstruction::U16(value));
                len += 2; 
                k+=2;
            }
            else {
                break;
            }
        }
        RiscvInstructions{buffer, len}
    }

    pub fn serialize(&self) -> Vec<u8> {

        let mut buffer = Vec::<u8>::new();

        for e in self.buffer.iter() {
            match e {
                RiscvInstruction::U16(x) => {
                    let bytes: [u8; 2] = x.to_be_bytes();
                    buffer.push(bytes[0]);
                    buffer.push(bytes[1]);
                },
                RiscvInstruction::U32(x) => {
                    let bytes: [u8; 4] = x.to_be_bytes();
                    buffer.push(bytes[0]);
                    buffer.push(bytes[1]);
                    buffer.push(bytes[2]);
                    buffer.push(bytes[3]);
                },
            }
        }

        return buffer;
    }

    pub fn len(&self) -> usize {
        self.len
    }
    
    pub fn count(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    //use rand::Rng;

    use rand::Rng;

    use crate::dasm::spike_dasm::SpikeDasm;
    use crate::dasm::{Dasm, DasmInstruction, InstructionType};
    //, InstructionType, RiscvInstructions};
    use crate::elf::ELF;

    use super::objdump_dasm::ObjdumpDasm;
    use super::RiscvInstructions;

    #[test]
    fn instruction_endianess() {

        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());

        let bytes_be: u32 = 0xBBCCDDEE;
        let bytes_le: u32 = bytes_be.swap_bytes();

        assert_eq!(bytes_le, 0xEEDDCCBB);

        assert_eq!(
            DasmInstruction::from_be(&mut disas, bytes_be, 0x0).unwrap(),
            DasmInstruction::from_le(&mut disas, bytes_le, 0x0).unwrap()
        );
    }


    #[test]
    fn instruction_bytes_endianess() {

        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());

        let bytes_be: [u8;4] = [0xBB, 0xCC, 0xDD, 0xEE];
        let bytes_le: [u8;4] = [0xEE, 0xDD, 0xCC, 0xBB];

        assert_eq!(
            DasmInstruction::from_be_bytes(&mut disas, bytes_be, 0x0).unwrap(),
            DasmInstruction::from_le_bytes(&mut disas, bytes_le, 0x0).unwrap()
        );
    }

    
    use rand::rngs::StdRng;
    use rand::SeedableRng;


    #[test]
    fn cpu_profileruction_layer() {

        /*

        [73] 3a 49 00 [aa] 00 [73] 3a 49 00 [73] 3a .. .. 
        |             |       |             |
        +=> 4B        +=> 2B  +=> 4B        +=> 4B

        This is the expected output:

        100084:       00493a73                csrrc   s4,uie,s2
        100088:       00aa                    slli    ra,ra,0xa
        10008a:       00493a73                csrrc   s4,uie,s2
        ...

        LE:
        
        73 3a 49 00  aa 00 73 3a 49 00 73 3a aa 00 49 00 aa 00 73 3a  aa 00 73 3a aa 00 49 00 73 3a aa 00
    
        */

        let be_bytes: [u8; 32] = [  0x00, 0x49, 0x3a, 0x73, 
                                    0x00, 0xaa, 
                                    0x00, 0x49, 0x3a, 0x73,
                                    0x3a, 0x73, 0x00, 0xaa,
                                    0x00, 0x49,
                                    0x00, 0xaa,
                                    0x00, 0xaa, 0x3a, 0x73,
                                    0x00, 0xaa, 0x3a, 0x73,
                                    0x00, 0x49,
                                    0x00, 0xaa, 0x3a, 0x73];

        let le_bytes: [u8; 32] = [  0x73, 0x3a, 0x49, 0x00,
                                    0xaa, 0x00,
                                    0x73, 0x3a, 0x49, 0x00,
                                    0x73, 0x3a, 0xaa, 0x00,
                                    0x49, 0x00,
                                    0xaa, 0x00,
                                    0x73, 0x3a, 0xaa, 0x00,
                                    0x73, 0x3a, 0xaa, 0x00,
                                    0x49, 0x00, 0x73, 0x3a,
                                    0xaa, 0x00];

        /*
            0000000000100084 <payload>:
            100084:       00493a73                csrrc   s4,uie,s2
            100088:       00aa                    slli    ra,ra,0xa
            10008a:       00493a73                csrrc   s4,uie,s2
            10008e:       00aa3a73                csrrc   s4,vxrm,s4
            100092:       0049                    c.nop   18
            100094:       00aa                    slli    ra,ra,0xa
            100096:       00aa3a73                csrrc   s4,vxrm,s4
            10009a:       00aa3a73                csrrc   s4,vxrm,s4
            10009e:       0049                    c.nop   18
            1000a0:       00aa3a73                csrrc   s4,vxrm,s4
        */

        let riscv_ins = RiscvInstructions::from_be(be_bytes.to_vec());
        assert!(riscv_ins.is_err());

        let riscv_ins1 = RiscvInstructions::from_be_lossy(be_bytes.to_vec());
        let riscv_ins2 = RiscvInstructions::from_le(le_bytes.to_vec());

        assert_eq!(riscv_ins1, riscv_ins2);
    }
}
