// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
use std::{fmt, ops::{Deref, DerefMut}, collections::HashMap};

use libafl_bolts::Error;

use crate::{disas::DisasDataTable, defines::read_lines};


#[derive(Debug, PartialEq, Eq, Hash)]
struct OpcodeData(Vec<u8>);

impl TryFrom<&str> for OpcodeData {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        if value.len() != 128 {
            Err("Opcode data string is expected to be 128 character in size!".to_string())
        } else {
            match hex::decode(value) {
                Ok(x) => Ok(OpcodeData(x)),
                Err(x) => Err(format!("Decoding error: {} / {}", x, value)),
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SpikeInstructionState {
    Executed,
    TrapInstructionAddressMisaligned,
    TrapInstructionAccessFault,
    TrapIllegalInstruction,
    TrapBreakpoint,
    TrapLoadAddressMisaligned,
    TrapStoreAddressMisaligned,
    TrapLoadAccessFault,
    TrapStoreAccessFault,
    TrapUserEcall,
    TrapSupervisorEcall,
    TrapVirtualSupervisorEcall,
    TrapMachineEcall,
    TrapInstructionPageFault,
    TrapLoadPageFault,
    TrapStorePageFault,
    TrapInstructionGuestPageFault,
    TrapLoadGuestPageFault,
    TrapVirtualInstruction,
    TrapStoreGuestPageFault,
}

impl fmt::Display for SpikeInstructionState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
       write!(f, "{:?}", self)
    }
}

impl From<&str> for SpikeInstructionState {
    fn from(item: &str) -> Self {
        match item {
            "trap_instruction_address_misaligned" => SpikeInstructionState::TrapInstructionAddressMisaligned,
            "trap_instruction_access_fault" => SpikeInstructionState::TrapInstructionAccessFault,
            "trap_illegal_instruction" => SpikeInstructionState::TrapIllegalInstruction,
            "trap_breakpoint" => SpikeInstructionState::TrapBreakpoint,
            "trap_load_address_misaligned" => SpikeInstructionState::TrapLoadAddressMisaligned,
            "trap_store_address_misaligned" => SpikeInstructionState::TrapStoreAddressMisaligned,
            "trap_load_access_fault" => SpikeInstructionState::TrapLoadAccessFault,
            "trap_store_access_fault" => SpikeInstructionState::TrapStoreAccessFault,
            "trap_user_ecall" => SpikeInstructionState::TrapUserEcall,
            "trap_supervisor_ecall" => SpikeInstructionState::TrapSupervisorEcall,
            "trap_virtual_supervisor_ecall" => SpikeInstructionState::TrapVirtualSupervisorEcall,
            "trap_machine_ecall" => SpikeInstructionState::TrapMachineEcall,
            "trap_instruction_page_fault" => SpikeInstructionState::TrapInstructionPageFault,
            "trap_load_page_fault" => SpikeInstructionState::TrapLoadPageFault,
            "trap_store_page_fault" => SpikeInstructionState::TrapStorePageFault,
            "trap_instruction_guest_page_fault" => SpikeInstructionState::TrapInstructionGuestPageFault,
            "trap_load_guest_page_fault" => SpikeInstructionState::TrapIllegalInstruction,
            "trap_virtual_instruction" => SpikeInstructionState::TrapIllegalInstruction,
            "trap_store_guest_page_fault" => SpikeInstructionState::TrapIllegalInstruction,
            x => {panic!("Unknown trap reason found: {}", x);},
        }
    }
}

pub const RISCV_TRAP_REASONS: &'static [SpikeInstructionState] = &[
    SpikeInstructionState::TrapInstructionAddressMisaligned,
    SpikeInstructionState::TrapInstructionAccessFault,
    SpikeInstructionState::TrapIllegalInstruction,
    SpikeInstructionState::TrapBreakpoint,
    SpikeInstructionState::TrapLoadAddressMisaligned,
    SpikeInstructionState::TrapStoreAddressMisaligned,
    SpikeInstructionState::TrapLoadAccessFault,
    SpikeInstructionState::TrapStoreAccessFault,
    SpikeInstructionState::TrapUserEcall,
    SpikeInstructionState::TrapSupervisorEcall,
    SpikeInstructionState::TrapVirtualSupervisorEcall,
    SpikeInstructionState::TrapMachineEcall,
    SpikeInstructionState::TrapInstructionPageFault,
    SpikeInstructionState::TrapLoadPageFault,
    SpikeInstructionState::TrapStorePageFault,
    SpikeInstructionState::TrapInstructionGuestPageFault,
    SpikeInstructionState::TrapLoadGuestPageFault,
    SpikeInstructionState::TrapVirtualInstruction,
    SpikeInstructionState::TrapStoreGuestPageFault,
];

#[derive(Debug)]
pub struct SpikeData {
    pub pc: u64,
    pub mnemonic: String,
    pub args: Option<String>,
    pub instr: u64,
    pub status: SpikeInstructionState,
}

#[derive(Debug)]
pub struct SpikeDataTable(Vec<SpikeData>);

impl Deref for SpikeDataTable {
    type Target = Vec<SpikeData>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SpikeDataTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl SpikeDataTable {

    pub fn from_spike_log(spike_log_file: &str, pc_input_start: u64, pc_input_end: u64, unique_pc: bool) ->  Result<Self, Error>{

        println!("READ: {}", spike_log_file);

        let mut result = Vec::<SpikeData>::new();
        let mut i = 0;

        let spike_log_file_lines = read_lines(spike_log_file)?;

        let mut pending_ins = false;

        let mut m = HashMap::<u64, Option<()>>::new();


        for line in spike_log_file_lines {
            let l = line?;

            if l.starts_with("(ins)") {
                pending_ins = false;

                if l.len() < 50 {
                    break;
                }

                let pc = <u64>::from_str_radix(&l[20..36], 16).unwrap();
                let instr = <u64>::from_str_radix(&l[40..48], 16).unwrap();
                let tmp = l[50..].to_string();

                let mnemonic = tmp.split(" ").into_iter().next().unwrap().to_string();

                let args = if (tmp.len()-mnemonic.len()) >= 1 {
                    Some(tmp[mnemonic.len()..].trim().to_string())
                }
                else {
                    None
                };

                if pc >= pc_input_start && pc <= pc_input_end {

                    if unique_pc {
                        if m.contains_key(&pc) {
                            continue;
                        }

                        m.insert(pc, None);
                    }
                
                    result.push(SpikeData {
                        pc: pc,
                        mnemonic: mnemonic,
                        instr: instr,
                        status: SpikeInstructionState::Executed,
                        args: args,
                    });
                    i += 1;
                    pending_ins = true;
                }
            }

            if l.starts_with("(exc)") && pending_ins {
                let trap_reason = l[26..].split(" ").next().unwrap();
                result[i-1].status = trap_reason.into();
            }
        }

        Ok(SpikeDataTable(result))
    }

    // fix me 
    pub fn from_spike_log_with_objdump(spike_log_file: &str, elf_file: &str, pc_input_start: u64, pc_input_end: u64, unique_pc: bool) -> Result<Self, Error>{

        let mut objdump_data_table = DisasDataTable::from_elf(elf_file,  pc_input_start, pc_input_end).unwrap();
        let mut result = Self::from_spike_log(spike_log_file, pc_input_start, pc_input_end, unique_pc)?;

        for e in result.iter_mut() {

            let offset = match objdump_data_table.get_index_by_address(e.pc) {
                Ok(x) => x,
                Err(_err) => {
                    eprintln!("Error: Instruction at PC:{:x} missing in objdump dissasembly (spike-status: {})?!", e.pc, e.status);
                    continue
                }
            };

            if e.mnemonic != objdump_data_table[offset].mnemonic {
                let objdump_pc = objdump_data_table.get_address_by_index(offset).unwrap();
                eprintln!("mismatch [spike/objdumo] {}/{} at {:x}/{:x} (spike-status: {})", e.mnemonic, objdump_data_table[offset].mnemonic, e.pc, objdump_pc, e.status);
            }
            e.mnemonic = objdump_data_table[offset].mnemonic.to_string();
        }

        return Ok(result);
    }
}
