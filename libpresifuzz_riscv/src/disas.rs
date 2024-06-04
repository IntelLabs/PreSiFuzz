// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, ops::{Deref, DerefMut}, str::FromStr};
use indexmap::IndexMap;
use libafl::Error;
use libafl_bolts::ErrorBacktrace;

use crate::dasm::{gen::{gen_cbeqz_instruction, gen_cbnez_instruction, gen_cj_instruction, gen_jal_instruction}, spike_dasm::SpikeDasm, Dasm, InstructionType, RiscvInstruction, RiscvInstructions};
use crate::dasm::gen::gen_branch_instruction;
use crate::dasm::DasmInstruction;
use crate::dasm::objdump_dasm::ObjdumpDasm;

#[derive(Debug, Clone)]
pub enum CofiType {
    
    /* InstructionType::CondBranchRelative */
    CBEQZ,
    CBNEZ,

    
    /* InstructionType::CondBranchRelativeCMP */
    BEQ, 
    BNE,
    BLT,
    BGE,
    BLTU,
    BGEU,

    /* InstructionType::BranchRelativeStore */
    J,      /* jr 0, offset */
    CJ,     /* compressed J */
    JAL,

    /* InstructionType::BranchAbsoluteStore */
    //CEBREAK /* ? */


}

impl FromStr for CofiType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "c.beqz" => Ok(CofiType::CBEQZ),
            "c.bnez" => Ok(CofiType::CBNEZ),

            "beq" => Ok(CofiType::BEQ),
            "bne" => Ok(CofiType::BNE),
            "blt" => Ok(CofiType::BLT),
            "bge" => Ok(CofiType::BGE),
            "bltu" => Ok(CofiType::BLTU),
            "bgeu" => Ok(CofiType::BGEU),

            /* pseudo-instructions */
            "beqz" => Ok(CofiType::BEQ),
            "bnez" => Ok(CofiType::BNE),
            "blez" => Ok(CofiType::BGE),
            "bgez" => Ok(CofiType::BGE),
            "bltz" => Ok(CofiType::BLT),
            "bgtz" => Ok(CofiType::BLT),

            "j" => Ok(CofiType::J),
            "c.j" => Ok(CofiType::CJ),
            "jal" => Ok(CofiType::JAL),

            //"c.ebreak" => Ok(CofiType::CEBREAK),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CofiTarget {
    TargetID(DisasId),
    TargetAbsoluteAddress(u64),
}

#[derive(Debug, Clone)]
pub struct CofiInstruction {
    //index: usize, // index in the base table
    
    /* values to craft the instruction */
    reg1: u8,
    reg2: u8,
    //btype: u8, /* TODO: use enum to support more types */

    cofi_type: CofiType,

    target: CofiTarget,
}

type DisasId = u64;
type DisasAddr = u64;

#[derive(Debug)]
pub struct DisasDataTable{

    disassembler: Box<dyn Dasm>,

    cur_id: DisasId, // object id
    pub table: IndexMap<DisasId, DasmInstruction>,
    /* hashmap to lookup PC -> table_offset */
    pub map: HashMap<DisasAddr, DisasId>,
    map_updated: bool,

    base_address: DisasAddr,

    /* total size of all instructions in bytes */
    _size: usize,

    /* list of all CoFIs "Change of Control Flow Instructions" that are pointing to other instructions in the payload buffer */
    cofi_table: IndexMap<DisasId, CofiInstruction>,
    /* map for CoFIs for resolving addresses to indices (needs to be updated once map_updated is set to false) */
    //cofi_map: HashMap<u64, usize>,

}

impl Deref for DisasDataTable {
    type Target = IndexMap<DisasId, DasmInstruction>;

    fn deref(&self) -> &Self::Target {
        &self.table
    }
}

impl DerefMut for DisasDataTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.table
    }
}

pub struct DisasDataTableIntoIterator<'a> {
    pos: usize,
    pc: u64,
    disas_ref: &'a DisasDataTable,
}

impl<'a> Iterator for DisasDataTableIntoIterator<'a> {
    type Item = (DisasId, DisasAddr, &'a DasmInstruction);

    fn next(&mut self) -> Option<Self::Item> {

        if self.pos >= self.disas_ref.table.len() {
            return None;
        }

        let pc = self.pc;
        let (id, object) = self.disas_ref.table.get_index(self.pos).unwrap();
        
        //[self.pos];

        /* check if the next instruction is a 2 byte padding nop */
        //if object.size == 4 && pc%4 != 0 {
        //    eprintln!("skip void 2 byte instruction at {:x}", pc);
        //    self.pc += 2;
        //    pc += 2;
        //}

        self.pc += self.disas_ref.table[self.pos].size as u64;
        self.pos += 1;

        return Some((*id, pc, object));
    }
}

impl<'a>  IntoIterator for &'a DisasDataTable {
    type Item = (DisasId, DisasAddr, &'a DasmInstruction);
    type IntoIter = DisasDataTableIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        DisasDataTableIntoIterator { 
            pos: 0,
            pc: self.base_address,
            disas_ref: self,
        }
    }
}


impl DisasDataTable {

    pub fn new(base_address: u64) ->  Self{
        let result = IndexMap::<DisasId, DasmInstruction>::new();
        let cofi_table = IndexMap::<DisasId, CofiInstruction>::new();
        let m = HashMap::<DisasAddr, DisasId>::new();
        //result.iter().enumerate().map(|(i,y)| (y.pc, i)).collect();

        //let mut cofi_table = Vec::<DasmInstruction>::new();
        // fix me
        //cofi_table.iter().enumerate().map(|(i,y)| (y.pc, i)).collect();

        DisasDataTable{
            disassembler: Box::new(SpikeDasm::new()),
            cur_id: 0,
            table: result,
            map: m,
            map_updated: false,
            _size: 0,
            cofi_table: cofi_table, //Some(cofi_table),
            //cofi_map: cofi_m, //Some(cofi_m),
            base_address: base_address,
        }
    }
    
    pub fn from_riscv_ins(input: &RiscvInstructions, base_address: u64) ->  Result<Self, Error>{
        let mut result = IndexMap::<DisasId, DasmInstruction>::new();
        let cofi_table = IndexMap::<DisasId, CofiInstruction>::new();
        let mut disas: Box<dyn Dasm> = Box::new(SpikeDasm::new());

        let instructions = disas.process_slice(input, base_address)?;


        let mut total_size = 0;
        let mut cur_id = 0;

        for (_, instruction) in &instructions {
            //println!("ins: {:?}", instruction);
            //result.insert(0, instruction);
            cur_id += 1;
            result.shift_insert(total_size, cur_id, instruction.clone());
            total_size += 1;
        }

        let map = HashMap::<DisasAddr, DisasId>::new();

        let mut ret = DisasDataTable {
            disassembler: disas,
            cur_id: cur_id,
            table: result,
            map: map,
            map_updated: false,
            _size: total_size,
            cofi_table: cofi_table, //Some(cofi_table),
            //cofi_map: cofi_m, //Some(cofi_m),
            base_address: base_address,
        };

        Self::populate_cofi_map(&mut ret, &instructions)?;

        Ok(ret)
    }
    
    /* Load entire ELF file, that might differs from a template file layout. */
    pub fn from_elf(elf_file: &str, base_address: u64, end_address: u64) ->  Result<Self, Error>{
        let objdump_disas = ObjdumpDasm::process_elf(elf_file, base_address, end_address).unwrap();
        let mut instructions = IndexMap::<DisasId, DasmInstruction>::with_capacity(objdump_disas.len());
        let cofi_table = IndexMap::<DisasId, CofiInstruction>::new();
        //let cofi_m: HashMap<u64, usize> = HashMap::<u64, usize>::new();

        let mut total_size = 0;
        let mut cur_id = 0;

        let mut prev = 0;
        let mut prev_size = 0;
        for (pc, instruction) in &objdump_disas {            
            if prev != 0 && (prev+prev_size) != *pc {
                panic!("Gap found at {:x} - fix me", pc);
            } 

            prev = *pc;
            prev_size = instruction.size as u64;
            //instructions.insert(0, instruction);

            cur_id += 1;
            instructions.shift_insert(total_size, cur_id, instruction.clone());
            total_size += 1;

        }

        let m = HashMap::<DisasAddr, DisasId>::new();

        let mut ret = DisasDataTable {
            disassembler: Box::new(SpikeDasm::new()),
            cur_id: 0,
            table: instructions,
            map: m,
            map_updated: false, // fix me
            _size: 0, // fix me!
            cofi_table: cofi_table,
            //cofi_map: cofi_m,
            base_address: base_address,
        };

        Self::populate_cofi_map(&mut ret, &objdump_disas)?;

        Ok(ret)
    }

    fn populate_cofi_map(cofi_table: &mut DisasDataTable, disas: &Vec<(u64, DasmInstruction)>) -> Result<(), Error> {
        for (address, instruction) in disas {
            //println!("ins: {:?}", instruction);

            match instruction.ins_type {
                InstructionType::CondBranchRelativeCMP(x) => {
                    let cofi_type =  CofiType::from_str(&instruction.mnemonic).unwrap();

                    /* TODO: check whether this will fail for target_address < address */
                    let target_address = ((*address as i64) + x.2 as i64) as u64;

                    let id = cofi_table.get_id_by_address(*address).unwrap();

                    let target_id = cofi_table.get_id_by_address(target_address);
                    if target_id.is_ok() {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetID(target_id.unwrap()), x.0, x.1, cofi_type)?;
                    }
                    else {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetAbsoluteAddress(target_address), x.0, x.1, cofi_type)?;
                    }
                }

                InstructionType::CondBranchRelative(x) => {
                    let cofi_type =  CofiType::from_str(&instruction.mnemonic).unwrap();

                    let target_address = ((*address as i64) + x.1 as i64) as u64;

                    let id = cofi_table.get_id_by_address(*address).unwrap();

                    let target_id = cofi_table.get_id_by_address(target_address);
                    if target_id.is_ok() {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetID(target_id.unwrap()), x.0, 0, cofi_type)?;
                    }
                    else {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetAbsoluteAddress(target_address), x.0, 0, cofi_type)?;
                    }
                }

                InstructionType::BranchRelativeStore(x) => {
        
                    let cofi_type =  CofiType::from_str(&instruction.mnemonic).unwrap();
                    let target_address = ((*address as i64) + x.1 as i64) as u64;

                    let id = cofi_table.get_id_by_address(*address).unwrap();
                    let target_id = cofi_table.get_id_by_address(target_address);
                    if target_id.is_ok() {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetID(target_id.unwrap()), x.0, 0, cofi_type)?;
                    }
                    else {
                        Self::add_cofi_object(&mut cofi_table.cofi_table, id, CofiTarget::TargetAbsoluteAddress(target_address), x.0, 0, cofi_type)?;
                    }

                    //println!("---> target_address: {:x} / {:?} / {}", target_address, instruction.ins_type, id);

                    //panic!("unimplemented: 0x{:x} - {:#?} {:#?}", address, x, instruction);
                }

                InstructionType::BranchAbsoluteStore(_) => {
                    /* absolute jumps are hard to implement as it depends on a register value (idea: introduce pseudo instruction to do both; set reg; jmp to address stored in reg) */
                    //panic!("unimplemented: 0x{:x} - {:#?} {:#?}", address, x, instruction);
                }

                _ => {
                    //panic!("TODO?!");
                }
            }
        }
        Ok(())
    }

    /* Check if the current lookup map is dirty and update it (there's probably a much more efficient way to do, but for now we keep it more simple and less efficient). */
    fn update_map(&mut self) {
        // fix this
        let m: HashMap<DisasAddr, DisasId> = self.into_iter().enumerate().map(|(_,(i, pc, _))| (pc as DisasAddr, i as DisasId)).collect();

        self.map = m;
        self.map_updated = true;
    }

    /* Wrapper methode that performance a check of the current state of the lookup map and returns a mutable reference. */
    fn get_lookup_map(&mut self) -> &mut HashMap<DisasAddr, DisasId>  {
        if !self.map_updated {
            self.update_map();
        }

        &mut self.map
    }

    /* Wrapper function to remove a given instruction by index (includes sanity checks and updates the lookup map state). */
    fn remove_instruction(&mut self, id: DisasId) -> Result<(), Error> {
        self.table.shift_remove(&id);
        self.cofi_table.shift_remove(&id);

        self.map_updated = false;
        return Ok(());
    }

    fn next_id(&mut self) -> DisasId {
        self.cur_id += 1;
        self.cur_id
    }

    fn reset_next_id(&mut self) {
        assert!(self.cur_id != 0);
        self.cur_id -= 1;
    }


    /* Add new instruction at a given index and update lookup map state. Return value is object ID. */
    fn add_instruction(&mut self, index: usize, instruction: DasmInstruction) -> Result<DisasId, Error>{

        if instruction.ins_type != InstructionType::Normal {
            return Err(Error::Unknown(format!("branch instruction not support ({:?})", instruction).to_string(), ErrorBacktrace::new()))
        }

        if index > self.table.len() {
            return Err(Error::Unknown(format!("index out of bounds => {}", index).to_string(), ErrorBacktrace::new()));
        }

        /* check if this is a cofi instruction */
        let next_id = self.next_id();
        self.table.shift_insert(index, next_id, instruction);
        //self.update_cofi_objects(index, true);
        self.map_updated = false;

        Ok(next_id)
    }

    /* Add new CoFI and update the list of CoFIs; includes fixing the index and target index value of each affected object. */
    fn add_cofi_object(cofi_table: &mut IndexMap<DisasId, CofiInstruction>, id: DisasId, target: CofiTarget, reg1: u8, reg2: u8, cofi_type: CofiType) -> Result<(), Error>{
        //self.update_cofi_objects(index, true);


        match cofi_type {

            CofiType::CJ => {
                cofi_table.insert(id, CofiInstruction{
                    //index,
                    reg1: 0,
                    reg2: 0,
                    //btype: 0,
                    cofi_type,
                    //target_id,
                    target,
                });
            } 

            /*
            c.beqz
            c.bnez
             */
            CofiType::CBEQZ | CofiType::CBNEZ |
            
            
            CofiType::J | CofiType::JAL
             => {
                cofi_table.insert(id, CofiInstruction{
                    //index,
                    reg1,
                    reg2: 0,
                    //btype: 0,
                    cofi_type,
                    //target_id,
                    target,
                });

            }

            CofiType::BEQ | CofiType::BGE | CofiType::BGEU | CofiType::BLT | CofiType::BLTU | CofiType::BNE => {
                cofi_table.insert(id, CofiInstruction{
                    //index,
                    reg1,
                    reg2,
                    //btype: branch_type,
                    cofi_type,
                    //target_id,
                    target,
                });
            }


            // _ => { panic!("unimplemented!")}

        }

        Ok(())
    }

    fn gen_cofi(disassembler: &mut Box<dyn Dasm>, address: u64, target_address: u64, reg1: u8, reg2: u8, cofi_type: &CofiType) -> Result<DasmInstruction, Error>{
        /* We need to calcualte the distance between the cofi instruction and the target instruction (need to add 2 or 4 bytes to the distance) */
        let distance = Self::distance(address, target_address, 4);

        //println!("distance: {}", distance);

        let instruction = match cofi_type {
            CofiType::CBEQZ  => {DasmInstruction::new_cbeqz (disassembler, address, reg1-8, distance as i16, false)},
            CofiType::CBNEZ  => {DasmInstruction::new_cbnez (disassembler, address, reg1-8, distance as i16, false)},

            CofiType::BEQ  => {DasmInstruction::new_beq (disassembler, address, reg1, reg2, distance, false)},
            CofiType::BNE  => {DasmInstruction::new_bne (disassembler, address, reg1, reg2, distance, false)},
            CofiType::BLT  => {DasmInstruction::new_blt (disassembler, address, reg1, reg2, distance, false)},
            CofiType::BGE  => {DasmInstruction::new_bge (disassembler, address, reg1, reg2, distance, false)},
            CofiType::BLTU => {DasmInstruction::new_bltu(disassembler, address, reg1, reg2, distance, false)},
            CofiType::BGEU => {DasmInstruction::new_bgeu(disassembler, address, reg1, reg2, distance, false)},

            CofiType::CJ    => {DasmInstruction::new_cj(disassembler, address, distance as i16, false)},
            CofiType::J    => {DasmInstruction::new_jal(disassembler, address, 0, distance, false)},
            CofiType::JAL    => {DasmInstruction::new_jal(disassembler, address, reg1, distance, false)},

            //CofiType::CEBREAK  => {panic!("CEBREAK")}

        }?;

        Ok(instruction)
    }

    fn add_cofi(&mut self, index: usize, target: CofiTarget, reg1: u8, reg2: u8, cofi_type: CofiType) -> Result<(), Error>{
        let address = self.get_address_by_index(index)?;

        let target_address = match target {
            CofiTarget::TargetID(x) => {self.get_address_by_id(x)?},
            CofiTarget::TargetAbsoluteAddress(address) => {address}
        };

        if index > self.table.len() {
            return Err(Error::Unknown("index out of bounds".to_string(), ErrorBacktrace::new()));
        }

        /* return an error if cofi_type is a compressed instruction and reg1 is not >= 8 && <= 15  */
        let reg1 = match cofi_type {
            CofiType::CBEQZ | CofiType::CBNEZ => {
                if reg1 > 8 {
                    return Err(Error::Unknown(format!("invalid reg1 specified for compressed instruction (reg1: {} / instruction: {:?})", reg1, cofi_type).to_string(), ErrorBacktrace::new()));
                }
                reg1 + 8
            }
            _ => {
                reg1
            }
        };
        

        let instruction = Self::gen_cofi(&mut self.disassembler, address, target_address, reg1, reg2, &cofi_type)?;

        let next_id = self.next_id();
        self.table.shift_insert(index, next_id, instruction);
        //self.table.insert(index as u64, instruction);
        let r = Self::add_cofi_object(&mut self.cofi_table, next_id, target, reg1, reg2, cofi_type);

        if r.is_err() {
            self.table.shift_remove(&next_id);
            self.reset_next_id();
            return r;
        }

        self.map_updated = false;

        Ok(())
    }

    fn distance(address: u64, target_address: u64, ins_size: usize) -> i32 {
        if address <= target_address {
            (target_address as i32 + ins_size as i32) - address as i32
        }
        else{
            target_address as i32 - address as i32
        }
    }

    fn distance_by_id(&mut self, id: DisasId, target_id: DisasId, ins_size: usize)  -> Result<i32, Error>{
        let address_a = self.get_address_by_id(id)?;
        let address_b = self.get_address_by_id(target_id)?;

        Ok(Self::distance(address_a, address_b, ins_size))
    }

    fn distance_by_id_absolute(&mut self, id: DisasId, target_address: u64, ins_size: usize)  -> Result<i32, Error>{
        let address_a = self.get_address_by_id(id)?;
        let address_b = target_address;

        Ok(Self::distance(address_a, address_b, ins_size))
    }

    /* Returns the current address of a given instruction by its index.
     * Supports the following two edge cases: 
     *  - returns the base_address if the list is empty 
     *  - returns the next possible address if index == self.table.len()
     * */
    pub fn get_address_by_index(&mut self, index: usize) -> Result<u64, Error>{
        if index > self.table.len() {
            return Err(Error::Unknown("index out of bounds".to_string(), ErrorBacktrace::new()));
        }

        // edge case 1
        if index == 0 {
            return Ok(self.base_address);
        }

        let mut last_address = self.base_address;

        for (idx, (_, address, instruction)) in self.into_iter().enumerate() {
            if idx == index {
                return Ok(address as u64);
            }
            last_address = address + instruction.size as u64;
        }

        // edge case 2
        if index == self.table.len() {
            return Ok(last_address);
        }

        return Err(Error::Unknown(format!("index not found ({})", index).to_string(), ErrorBacktrace::new()));
    }

    pub fn get_address_by_id(&mut self, id: DisasId) -> Result<u64, Error>{
        //let mut last_address = self.base_address;

        for (i, address, _instruction) in self.into_iter(){
            if id == i {
                return Ok(address as u64);
            }
            //last_address = address + instruction.size as u64;
        }

        return Err(Error::Unknown(format!("id not found ({})", id).to_string(), ErrorBacktrace::new()));
    }

    pub fn get_id_by_address(&mut self, address: u64) -> Result<DisasId, Error>{
        match self.get_lookup_map().get(&address) {
            Some(x) => Ok(*x),
            None => {
                Err(Error::Unknown(format!("instruction at address 0x{:x} not found", address).to_string(), ErrorBacktrace::new()))
            }
        }
    }

    pub fn get_id_by_index(&mut self, index: usize) -> Result<DisasId, Error>{
        let (id,_) = self.get_by_index(index)?;
        Ok(id) 
    }


    /* Returns the current index of a given instruction by its address. */
    pub fn get_index_by_address(&mut self, address: u64) -> Result<usize, Error>{
        let id = self.get_id_by_address(address)?;
        match self.table.get_index_of(&id) {
            Some(x) => { 
                Ok(x)
            }
            None => {
                Err(Error::Unknown(format!("instruction at address 0x{:x} not found", address).to_string(), ErrorBacktrace::new()))
            }
        }
    }

    fn _get_by_id(table: &mut IndexMap<DisasId, DasmInstruction>, id: DisasId) -> Result<(DisasId, DasmInstruction), Error>{
        match table.get_key_value(&id) {
            Some((x,y)) => {
                Ok((*x, y.clone()))
            }
            None => {
                return Err(Error::Unknown(format!("id: {} not found!", id).to_string(), ErrorBacktrace::new()));
            }
        }
    }

    pub fn get_by_id(&mut self, id: DisasId) -> Result<(DisasId, DasmInstruction), Error>{
        Self::_get_by_id(&mut self.table, id)
    }

    pub fn get_by_index(&mut self, index: usize) -> Result<(DisasId, DasmInstruction), Error>{
        match self.table.get_index(index) {
            Some((x,y)) => {
                Ok((*x, y.clone()))
            }
            None => {
                return Err(Error::Unknown(format!("index: {} not found!", index).to_string(), ErrorBacktrace::new()));
            }
        }
    }

    /* 
    pub fn get_by_address(&mut self, address: u64) -> Result<(u64, DasmInstruction), Error>{
        match self.table.get_index(index) {
            Some((x,y)) => {
                Ok((*x, y.clone()))
            }
            None => {
                return Err(Error::Unknown(format!("index: {} not found!", index).to_string(), ErrorBacktrace::new()));
            }
        }
    }
    */

    /*
    pub fn get_address_by_id(&mut self, id: u64) -> Result<u64, Error>{

    }
    */

    
    

    /* Add new instruction at a given index. */
    pub fn add_instruction_at_index(&mut self, index: usize, instr: u32) -> Result<DisasId, Error>{
        let address = self.get_address_by_index(index)?;
        let instruction = DasmInstruction::from_be(&mut self.disassembler, instr, address)?;
        self.add_instruction(index, instruction)
    }

    /* Add new instruction at a given index. */
    pub fn add_instruction_by_address(&mut self, address: u64, instr: u32) -> Result<DisasId, Error>{
        let index = self.get_index_by_address(address)?;
        let instruction = DasmInstruction::from_be(&mut self.disassembler, instr, address)?;
        self.add_instruction(index, instruction)
    }

    /* Placeholder for other CoFi instructions */
    pub fn add_branch_instruction_by_index(&mut self, index: usize, target: CofiTarget, reg1: u8, reg2: u8, cofi_type: CofiType) -> Result<(), Error>{
        self.add_cofi(index, target, reg1, reg2, cofi_type)?;
        Ok(())
    }

    pub fn remove_instruction_by_index(&mut self, index: usize) -> Result<(), Error>{
        let id = self.get_id_by_index(index)?;
        self.remove_instruction(id)
    }

    pub fn remove_instruction_by_address(&mut self, address: u64) -> Result<(), Error>{
        let id = self.get_id_by_address(address)?;
        self.remove_instruction(id)
    }

    fn serialize_cofi(&mut self, id: DisasId, safe: bool) -> Option<(u32, usize)> {
        let cofi = (*self.cofi_table.get(&id).unwrap()).clone();


        let reg1 = cofi.reg1;
        let reg2 = cofi.reg2;
        //let index = cofi.index;

        let distance = match cofi.target {
            CofiTarget::TargetID(x) => {
                let target_id = x;
                self.distance_by_id(id, target_id, 0).unwrap()
            },
            CofiTarget::TargetAbsoluteAddress(target_address) => {
                self.distance_by_id_absolute(id, target_address, 0).unwrap()
            }
        };

        match &cofi.cofi_type {
            CofiType::CBEQZ => {
                match gen_cbeqz_instruction(reg1-8, distance as i16, safe) {
                    Ok(x) => Some((x.0 as u32, x.1)),
                    Err(_) => None,
                }
            },

            CofiType::CBNEZ => {
                match gen_cbnez_instruction(reg1-8, distance as i16, safe) {
                    Ok(x) => Some((x.0 as u32, x.1)),
                    Err(_) => None,
                }
            },
    
            CofiType::BEQ | CofiType::BGE | CofiType::BGEU | CofiType::BLT | CofiType::BLTU | CofiType::BNE => {

                let btype = match cofi.cofi_type {
                    CofiType::BEQ => 0,
                    CofiType::BNE => 1,
                    CofiType::BLT => 4,
                    CofiType::BGE => 5,
                    CofiType::BLTU => 6,
                    CofiType::BGEU => 7,
                    _ => { panic!("unimplemented!")}
                };

                match gen_branch_instruction(reg1, reg2, distance, btype, safe) {
                    Ok(x) => Some(x),
                    Err(_) => None,
                }
            }
            CofiType::J   => {            
                match gen_jal_instruction(0, distance, safe) {
                    Ok(x) => Some(x),
                    Err(_) => None,
                }
            }
            CofiType::JAL => {
                match gen_jal_instruction(reg1, distance, safe) {
                    Ok(x) => Some(x),
                    Err(_) => None,
                }
            }
            CofiType::CJ  => {
                match gen_cj_instruction(distance as i16, safe) {
                    Ok(x) => Some((x.0 as u32, x.1)),
                    Err(_) => None,
                }
            }
        }
    }

    fn serialize_cofi_32(&mut self, id: DisasId, safe: bool) -> Option<u32> {
        match self.serialize_cofi(id, safe) {
            Some((v, s)) => {
                assert_eq!(s,4);
                Some(v)
            }
            None => {
                None
            }
        }
    }

    fn serialize_cofi_16(&mut self, id: DisasId, safe: bool) -> Option<u16> {
        match self.serialize_cofi(id, safe) {
            Some((v, s)) => {
                assert_eq!(s,2);
                Some(v as u16)
            }
            None => {
                None
            }
        }
    } 

    fn fetch32(&mut self, index: usize, safe: bool) -> Option<u32> {
        match &self.table[index].ins_type {
            InstructionType::Normal | InstructionType::BranchAbsoluteStore(_) => // todo: update once we support branchAbsoluteStore instructions 
            {
                Some(self.table[index].bytes as u32)
            }
            InstructionType::CondBranchRelative(_) | InstructionType::CondBranchRelativeCMP(_) | InstructionType::BranchRelativeStore(_) => {
                let id = self.get_id_by_index(index).unwrap();
                self.serialize_cofi_32(id, safe)
            }
    }
    }

    fn fetch16(&mut self, index: usize, safe: bool) -> Option<u16> {
        match &self.table[index].ins_type {
            InstructionType::Normal | InstructionType::BranchAbsoluteStore(_) => // todo: update once we support branchAbsoluteStore instructions 
            {
                Some(self.table[index].bytes as u16)  
            }
            InstructionType::CondBranchRelative(_) | InstructionType::CondBranchRelativeCMP(_) | InstructionType::BranchRelativeStore(_) => {
                let id = self.get_id_by_index(index).unwrap();
                self.serialize_cofi_16(id, safe)
            }
        }
    }

    pub fn _serialize(&mut self, safe: bool) -> RiscvInstructions {
        let mut buffer = RiscvInstructions::new();

        let mut idx = 0;
        while idx < self.table.len() {

            if self.table[idx].size == 4 {
                match self.fetch32(idx, safe) { 
                    Some(bytes) => {
                        buffer.push(RiscvInstruction::from_be32(bytes));
                    }
                    _ => {}
                };
                idx += 1;
            }
            else if self.table[idx].size == 2 {
                match self.fetch16(idx, safe) { 
                    Some(bytes) => {
                        buffer.push(RiscvInstruction::from_be16(bytes).unwrap());
                    }
                    _ => {}
                };
                idx += 1;
            }
            else {
                unimplemented!("todo");
            }
        }

        buffer
    }

    pub fn serialize(&mut self) -> RiscvInstructions {
        self._serialize(false)
    }

    pub fn serialize_safe(&mut self) -> RiscvInstructions {
        self._serialize(true)
    }


}
