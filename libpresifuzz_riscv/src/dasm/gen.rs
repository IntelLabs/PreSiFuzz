// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use libafl::Error;
use libafl_bolts::ErrorBacktrace;

pub fn gen_jal_instruction(reg: u8, offset: i32, safe: bool) -> Result<(u32, usize), Error> {
    if reg > 31 {
        return Err(Error::Unknown("reg > 31".to_string(), ErrorBacktrace::new()));
    }

    if safe && !(offset%2 == 0 && offset < (1024*1024) && offset >= (-1*1024*1024)) {
        return Err(Error::Unknown(format!("unsupported offset {:x}", offset).to_string(), ErrorBacktrace::new()));
    }

    let mut v: u32 = 0x6F;
    v |= (reg as u32) << 7; 

    v |= ((offset as u32 >> 12) & 0xFF) << 12;
    v |= ((offset as u32 >> 11) & 1) << 20;
    v |= ((offset as u32 >> 1) & 0x3FF) << 21;
    v |= ((offset as u32 >> 20) & 1) << 31;

    Ok((v, 4))
}

pub fn gen_jalr_instruction(reg1: u8, reg2: u8, offset: i32) -> Result<(u32, usize), Error> {
    if reg1 > 31 {
        return Err(Error::Unknown("reg1 > 31".to_string(), ErrorBacktrace::new()));
    }

    if reg2 > 31 {
        return Err(Error::Unknown("reg2 > 31".to_string(), ErrorBacktrace::new()));
    }

    let mut v: u32 = 0x67;

    /* target address is stored in rd */
    v |= (reg1 as u32) << 7; 
    v |= (reg2 as u32) << 15; 

    /* offset is only 2**12 in size (-2048 - 2048) */
    v |= (offset as u32 & 0xFFF) << 20; 

    Ok((v, 4))
}

pub fn gen_cj_instruction(offset: i16, safe: bool) -> Result<(u16, usize), Error> {
    let mut v: u16 = 0xA001;

    if safe && !(offset%2 == 0 && offset < 2048 && offset >= -2048) {
        return Err(Error::Unknown(format!("unsupported offset {:x}", offset).to_string(), ErrorBacktrace::new()));
    }

    v |= ((offset as u16 >> 5) & 1) << 2; 
    v |= ((offset as u16 >> 1) & 0x7) << 3; 
    v |= ((offset as u16 >> 7) & 0x1) << 6; 
    v |= ((offset as u16 >> 6) & 0x1) << 7; 
    v |= ((offset as u16 >> 10) & 0x1) << 8; 
    v |= ((offset as u16 >> 8) & 0x3) << 9; 
    v |= ((offset as u16 >> 4) & 0x1) << 11; 
    v |= ((offset as u16 >> 11) & 0x1) << 12; 

    Ok((v, 2))
}

pub fn gen_cjr_instruction(reg: u8) -> Result<(u16, usize), Error> {
    let mut v: u16 = 0x8002;

    if reg > 31 {
        return Err(Error::Unknown("reg1 > 31".to_string(), ErrorBacktrace::new()));
    }

    v |= (reg as u16) << 7; 

    Ok((v, 2))
}

pub fn gen_cjalr_instruction(reg: u8) -> Result<(u16, usize), Error> {
    let mut v: u16 = 0x9002;

    if reg > 31 {
        return Err(Error::Unknown("reg1 > 31".to_string(), ErrorBacktrace::new()));
    }

    v |= (reg as u16) << 7; 

    Ok((v, 2))
}

pub fn gen_branch_instruction(reg1: u8, reg2: u8, offset: i32, btype: u8, safe: bool) -> Result<(u32, usize), Error> {
    if reg1 > 31 {
        return Err(Error::Unknown("reg1 > 31".to_string(), ErrorBacktrace::new()));
    }
    if reg2 > 31 {
        return Err(Error::Unknown("reg2 > 31".to_string(), ErrorBacktrace::new()));
    }

    let mut v: u32 = 0x63;

    match btype {
        0 => {
            v |= 0<<12; // beq
        }
        1 => {
            v |= 1<<12; // bne
        }
        4 => {
            v |= 4<<12; // blt
        }
        5 => {
            v |= 5<<12; // bge
        }
        6 => {
            v |= 6<<12; // bltu
        }
        7 => {
            v |= 7<<12; // bgeu
        }
        _ => {
            return Err(Error::Unknown(format!("unsupported btype ({})", btype).to_string(), ErrorBacktrace::new()));
        }
    }

    if safe && !(offset%2 == 0 && offset < 4096 && offset >= -4096) {
        return Err(Error::Unknown(format!("unsupported offset {:x}", offset).to_string(), ErrorBacktrace::new()));
    }

    v |= (reg1 as u32) << 15; 
    v |= (reg2 as u32) << 20; 

    let offset13 = (offset as u32) >> 1; 

    v |= (offset13 & 0xF) << 8; 
    v |= ((offset13 & 0x3F0) >> 4) << 25; 

    v |= ((offset13 & 0x400) >> 10) << 7; 
    v |= ((offset13 & 0x800) >> 11) << 31; 

    Ok((v, 4))
}

pub fn gen_cbeqz_instruction(reg: u8, offset: i16, safe: bool) -> Result<(u16, usize), Error> {
    let mut v: u16 = 0xC001;

    if reg > 7 {
        return Err(Error::Unknown(format!("reg > 7 (value: {})", reg).to_string(), ErrorBacktrace::new()));
    }

    if safe && !(offset%2 == 0 && offset < 256 && offset >= -256) {
        return Err(Error::Unknown(format!("unsupported offset {:x}", offset).to_string(), ErrorBacktrace::new()));
    }

    v |= (reg as u16) << 7; 


    v |= ((offset as u16 >> 5) & 0x1) << 2; 
    v |= ((offset as u16 >> 1) & 0x3) << 3; 
    v |= ((offset as u16 >> 6) & 0x3) << 5; 

    v |= ((offset as u16 >> 3) & 0x3) << 10; 
    v |= ((offset as u16 >> 8) & 0x1) << 12; 


    Ok((v, 2))
}

pub fn gen_cbnez_instruction(reg: u8, offset: i16, safe: bool) -> Result<(u16, usize), Error> {
    let mut v: u16 = 0xE001;

    if reg > 7 {
        return Err(Error::Unknown("reg > 7".to_string(), ErrorBacktrace::new()));
    }

    if safe && !(offset%2 == 0 && offset < 256 && offset >= -256) {
        return Err(Error::Unknown(format!("unsupported offset {:x}", offset).to_string(), ErrorBacktrace::new()));
    }

    v |= (reg as u16) << 7; 

    v |= ((offset as u16 >> 5) & 0x1) << 2; 
    v |= ((offset as u16 >> 1) & 0x3) << 3; 
    v |= ((offset as u16 >> 6) & 0x3) << 5; 

    v |= ((offset as u16 >> 3) & 0x3) << 10; 
    v |= ((offset as u16 >> 8) & 0x1) << 12; 

    Ok((v, 2))
}

#[cfg(test)]
mod test {
    use rand::Rng;
    use crate::dasm::{spike_dasm::SpikeDasm, InstructionType, Dasm, DasmInstruction};


    #[test]
    fn craft_16_jmp_instructions() {
        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());
        let mut rng = rand::thread_rng();

        /* c.j */
        for _ in 0..128 {
            let offset = rng.gen_range(-2048..2048) as i16 & !1;
            let instruction = DasmInstruction::new_cj(&mut disas, 0x1000, offset, false).unwrap(); 

            assert_eq!(instruction.mnemonic, "c.j");
            assert_eq!(instruction.ins_type, InstructionType::BranchRelativeStore((0, offset as i32)));
        }        

        /* c.jr */
        for _ in 0..128 {
            let reg = rng.gen_range(0..31) as u8;
            let instruction = DasmInstruction::new_cjr(&mut disas, 0x1000, reg).unwrap(); 

            if reg == 1 {
                assert_eq!(instruction.mnemonic, "ret");
            }
            else {
                assert_eq!(instruction.mnemonic, "c.jr");
            }
            assert_eq!(instruction.ins_type, InstructionType::BranchAbsoluteStore((0, reg, 0)));
        }    

        /* c.jalr */
        for _ in 0..128 {
            let reg = rng.gen_range(0..31) as u8;
            let instruction = DasmInstruction::new_cjalr(&mut disas, 0x1000, reg).unwrap(); 


            if reg == 0 {
                assert_eq!(instruction.mnemonic, "c.ebreak");
            }
            else { 
                assert_eq!(instruction.mnemonic, "c.jalr");
            }

            assert_eq!(instruction.ins_type, InstructionType::BranchAbsoluteStore((0, reg, 0)));
        }    

    }

    #[test]
    fn craft_16_branch_instructions() {

        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());
        let mut rng = rand::thread_rng();


        /* c.beqz */
        for _ in 0..128 {
            let reg = rng.gen_range(0..7) as u8;
            let offset = rng.gen_range(-128..128) as i16 & !1;
            let instruction = DasmInstruction::new_cbeqz(&mut disas, 0x1000, reg, offset, false).unwrap(); 
            assert_eq!(instruction.mnemonic, "c.beqz");
            assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg+8, offset as i32)));
        }

        /* c.bnez */
        for _ in 0..128 {
            let reg = rng.gen_range(0..7) as u8;
            let offset = rng.gen_range(-128..128) as i16 & !1;
            let instruction = DasmInstruction::new_cbnez(&mut disas, 0x1000, reg, offset, false).unwrap(); 
            assert_eq!(instruction.mnemonic, "c.bnez");
            assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg+8, offset as i32)));
        }
    }


    #[test]
    fn craft_32_jmp_instructions() {
        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());
        let mut rng = rand::thread_rng();

        /* jal */
        for _ in 0..128 {
            let reg = rng.gen_range(0..31) as u8;

            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_jal(&mut disas, 0x1000, reg, offset, false).unwrap(); 


            if reg == 0{
                assert_eq!(instruction.mnemonic, "j");
            }
            else {
                assert_eq!(instruction.mnemonic, "jal");                
            }

            assert_eq!(instruction.ins_type, InstructionType::BranchRelativeStore((reg, offset)));
        }        

        /* jalr */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;

            let offset = rng.gen_range(-2048..2048) as i32 & !1;
            let instruction = DasmInstruction::new_jalr(&mut disas, 0x1000, reg1, reg2, offset).unwrap(); 

            assert_eq!(instruction.mnemonic, "jalr");                
            assert_eq!(instruction.ins_type, InstructionType::BranchAbsoluteStore((reg1, reg2, offset)));
        }
    }


    #[test]
    fn craft_32_branch_instructions() {

        let mut disas: Box<dyn Dasm>= Box::new(SpikeDasm::new());
        let mut rng = rand::thread_rng();


        /* beq */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_beq(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 

            if reg2 == 0{
                assert_eq!(instruction.mnemonic, "beqz");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
                assert_eq!(instruction.ins_type, DasmInstruction::new_beqz(&mut disas, 0x1000, reg1, offset, false).unwrap().ins_type);
            }
            else {
                assert_eq!(instruction.mnemonic, "beq");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

        /* bne */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_bne(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 
            
            if reg2 == 0{
                assert_eq!(instruction.mnemonic, "bnez");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
                assert_eq!(instruction.ins_type, DasmInstruction::new_bnez(&mut disas, 0x1000, reg1, offset, false).unwrap().ins_type);
            }
            else {
                assert_eq!(instruction.mnemonic, "bne");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

        /* blt */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_blt(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 
            
            if reg2 == 0{
                assert_eq!(instruction.mnemonic, "bltz");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
                assert_eq!(instruction.ins_type, DasmInstruction::new_bltz(&mut disas, 0x1000, reg1, offset, false).unwrap().ins_type);

            }
            else {
                assert_eq!(instruction.mnemonic, "blt");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

        /* bge */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_bge(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 
            
            if reg2 == 0{
                assert_eq!(instruction.mnemonic, "bgez");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
                assert_eq!(instruction.ins_type, DasmInstruction::new_bgez(&mut disas, 0x1000, reg1, offset, false).unwrap().ins_type);
            }
            else {
                assert_eq!(instruction.mnemonic, "bge");
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

        /* bltu */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_bltu(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 
            
            assert_eq!(instruction.mnemonic, "bltu");
            if reg2 == 0{
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
            }
            else {
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

        /* bgeu */
        for _ in 0..128 {
            let reg1 = rng.gen_range(0..31) as u8;
            let reg2 = rng.gen_range(0..31) as u8;
            let offset = rng.gen_range(-4096..4096) as i32 & !1;
            let instruction = DasmInstruction::new_bgeu(&mut disas, 0x1000, reg1, reg2, offset, false).unwrap(); 
            
            assert_eq!(instruction.mnemonic, "bgeu");
            if reg2 == 0{
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelative((reg1, offset)));
            }
            else {
                assert_eq!(instruction.ins_type, InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)));
            }
        }

    }
}
