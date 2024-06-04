// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::io::Read;
use std::io::Write;
use std::process::Command;
use std::process::Child;
use std::process::Stdio;
use libafl::Error;
use libafl_bolts::ErrorBacktrace;

use super::Dasm;
use super::DasmInstruction;
use super::helper::process_branch;
use super::RiscvInstruction;
use super::RiscvInstructions;

#[derive(Debug)]
pub struct SpikeDasm {
    process: Child,
}

impl SpikeDasm {

    fn _start_process() -> Child {
        Command::new("spike-dasm")
            .stdout(Stdio::piped())
            .stdin(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("failed to start process")
    }

    pub fn new() -> Self {

        SpikeDasm {
            process: Self::_start_process()
        }
    }

}

impl Dasm for SpikeDasm {


    /* Returns an Option of tuple of mnemonic, Option<args>, instruction size (2 or 4 bytes)  */
    fn process_single(&mut self, ins: &RiscvInstruction, address: u64)  -> Result<DasmInstruction, Error> {
        let arg = format!("DASM({:x})\n", ins.be_value());

        //println!("=> {:#?} {:x}", ins, ins.be_value());

        let instruction_size = match ins {
            RiscvInstruction::U16(_) => { 2 },
            RiscvInstruction::U32(_) => { 4 },
        };

        let arg_bytes = arg.as_bytes();
        if let Some(child_stdin) = self.process.stdin.as_mut() {
            child_stdin.write(arg_bytes).unwrap_or_default();
        }

        let mut output = [0; 1024];

        if let Some(child_stdout) = self.process.stdout.as_mut() {
            child_stdout.read(&mut output).unwrap();
            let stdout_data_raw = String::from_utf8_lossy(&output);

            let stdout_data = stdout_data_raw.split("\x00").take(1).collect::<Vec<&str>>()[0];
            
            let mnemonic = stdout_data.trim().split(' ').next().unwrap_or_default().to_string();

            let args = if stdout_data[mnemonic.len()..].trim().len() >= 1 {
                Some(stdout_data[mnemonic.len()..].trim().to_string())
            }
            else {

                let ins_type = process_branch(&mnemonic, None, address);

                return Ok(DasmInstruction{
                    mnemonic: mnemonic,
                    args: None,
                    bytes: if instruction_size == 2 { ins.be_value() & 0xFFFF } else { ins.be_value() } as u64,
                    size: instruction_size,
                    ins_type: ins_type,
                    }
                );
            };

            let ins_type = process_branch(&mnemonic, args.clone(), address);

            return Ok(DasmInstruction{
                mnemonic: mnemonic,
                args: args,
                bytes: if instruction_size == 2 { ins.be_value() & 0xFFFF } else { ins.be_value() } as u64,
                size: instruction_size,
                ins_type: ins_type,
            }
            );
        }
        else {
            return Err(Error::Unknown("spike-dasm failed (cannot acquire stdout))?".to_string(), ErrorBacktrace::new()))
        }
    }

    fn process_slice(&mut self, ins: &RiscvInstructions, address: u64) -> Result<Vec<(u64, DasmInstruction)>, Error> {

        let mut ret = Vec::<(u64, DasmInstruction)>::new();
        let mut cur_address = address; 


        for i in ins.iter() {
            let t = self.process_single(i, cur_address)?;
            ret.push((cur_address, t));


            cur_address += match i {
                RiscvInstruction::U16(_) => 2,
                RiscvInstruction::U32(_) => 4,
            } as u64;
            
        }

        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use crate::dasm::{Dasm, DasmInstruction, InstructionType, RiscvInstruction, RiscvInstructions};

    use super::SpikeDasm;
    use rand::Rng;


    #[test]
    fn spike_dasm_single() {

        let mut rng = rand::thread_rng();
        let mut spike_dasm = SpikeDasm::new();

        //let bytes: Vec<u8> = vec![0, 0x49]; 

        for _ in 0..4096 {
            let value = rng.gen::<u32>();
            let _ = spike_dasm.process_single(&RiscvInstruction::from_be32(value), 0x1000);
            //println!("DASM({:x}) -> {:?}", value, a);
        }

        //println!("{:?}", RiscvInstruction::from_be32(0x49));
        //println!("{:?}", RiscvInstruction::from_be32(0x49).be_value());

        assert_eq!(spike_dasm.process_single(&RiscvInstruction::from_be32(0x49), 0x1000).unwrap(), DasmInstruction{
            mnemonic: "c.addi".to_string(),
            args: Some("x0, 18".to_string()),
            bytes: 0x49,
            size: 2,
            ins_type: InstructionType::Normal,
        });
        assert_eq!(spike_dasm.process_single(&RiscvInstruction::from_be32(0xAA), 0x1000).unwrap(), DasmInstruction{
            mnemonic: "c.slli".to_string(),
            args: Some("ra, 10".to_string()),
            bytes: 0xAA,
            size: 2,
            ins_type: InstructionType::Normal,
        });
        
        assert_eq!(spike_dasm.process_single(&RiscvInstruction::from_be32(0xBB), 0x1000).unwrap(), DasmInstruction{
            mnemonic: "addw".to_string(),
            args: Some("ra, x0, x0".to_string()),
            bytes: 0xBB,
            size: 4,
            ins_type: InstructionType::Normal,
        });

    }

    #[test]
    fn spike_dasm_slice() {
        let mut spike_dasm = SpikeDasm::new();

        let mut input = Vec::<u8>::new();

        let mut bytes_1: Vec<u8> = vec![0, 0, 0, 0xBB];
        let mut bytes_2: Vec<u8> = vec![0, 0xAA];
        let mut bytes_3: Vec<u8> = vec![0, 0x49];

        input.append(&mut bytes_1);
        input.append(&mut bytes_2);
        input.append(&mut bytes_3);

        let riscv_ins = RiscvInstructions::from_be(input).unwrap();
        let _ = spike_dasm.process_slice(&riscv_ins, 0x1000).unwrap();
        let instructions = spike_dasm.process_slice(&riscv_ins, 0x1000).unwrap();

        assert_eq!(instructions.len(), 3);

        //for i in &instructions {
        //    println!("instruction: {:?}", i);
        //
        //}

        assert_eq!(instructions[0], (0x1000, DasmInstruction{
            mnemonic: "addw".to_string(),
            args: Some("ra, x0, x0".to_string()),
            bytes: 0xBB,
            size: 4,
            ins_type: InstructionType::Normal,
        }));
        assert_eq!(instructions[1], (0x1004, DasmInstruction{
            mnemonic: "c.slli".to_string(),
            args: Some("ra, 10".to_string()),
            bytes: 0xAA,
            size: 2,
            ins_type: InstructionType::Normal,
        }));
        
        assert_eq!(instructions[2], (0x1006, DasmInstruction{
            mnemonic: "c.addi".to_string(),
            args: Some("x0, 18".to_string()),
            bytes: 0x49,
            size: 2,
            ins_type: InstructionType::Normal,
        }));


    }

    #[test]
    fn spike_dasm_slice_unaligned() {
        let mut spike_dasm = SpikeDasm::new();

        let mut input = Vec::<u8>::new();

        let mut bytes_1: Vec<u8> = vec![0, 0xAA];
        let mut bytes_2: Vec<u8> = vec![0, 0, 0, 0xBB];
        let mut bytes_3: Vec<u8> = vec![0, 0x49];

        input.append(&mut bytes_1);
        input.append(&mut bytes_2);
        input.append(&mut bytes_3);

        let riscv_ins = RiscvInstructions::from_be(input).unwrap();

        /* this should fail */
        let instructions = spike_dasm.process_slice(&riscv_ins, 0x1000).unwrap();

        // the disassembler will fail here on the second unaligned instruction (as a result it will put a fourth instruction in between to fix the alignment)
        assert_eq!(instructions.len(), 3);
        //assert_eq!(instructions[1].1.mnemonic, "c.unimp".to_string());

        //println!("=============");
        //for i in &instructions {
        //    println!("instruction: {:?}", i);
        //}
        

        /* this should pass ?! */
        let instructions = spike_dasm.process_slice(&riscv_ins, 0x1002).unwrap();

        // we expect the API to return 3 functions if the address alignment matches with the alignment present in the slice 
        assert_eq!(instructions.len(), 3);

        //println!("=============");
        //for i in &instructions {
        //    println!("instruction: {:?}", i);
        //}
    }

    #[test]
    fn spike_dasm_random_slice() {

        let mut rng = rand::thread_rng();

        let mut objdump_dasm = SpikeDasm::new();

        for _ in 0..16 {
            let input: Vec<u8> = (0..1024).map(|_| rng.gen_range(0..255)).collect();
            let riscv_ins = RiscvInstructions::from_be_lossy(input);

            let _ = objdump_dasm.process_slice(&riscv_ins, 0x1000).unwrap();
        }

        for _ in 0..16 {
            let input: Vec<u8> = (0..1024).map(|_| rng.gen_range(0..255)).collect();
            let riscv_ins = RiscvInstructions::from_le(input);

            let _ = objdump_dasm.process_slice(&riscv_ins, 0x1000).unwrap();
        }
    }
}
