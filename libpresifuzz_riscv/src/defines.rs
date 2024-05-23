// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::cpu_profile::ALL_RISCV_INSTR;
use std::{collections::HashMap, fs::File, path::Path, io::{self, BufRead}};

const RISCV_COFI:  &'static [&'static str] = &[
    /* conditional branch instructions */
    "c.beqz", "c.bnez", "beq", "beqz", "bge", "bgeu", "bgez", "blt", "bltu", "bltz", "bne", "bnez",
    
    /* unconditional relative jumps */
    "c.j", "j", "jal",

    /* far jumps */
    "c.jalr", "c.jr", "jalr", "jr",

    /* far transfer (not sure if we need to handle them to be honest) */
    //"ret", "mret"
];

lazy_static! {

    pub static ref ALL_RISCV_INSTR_MAP: HashMap<&'static str, usize> = ALL_RISCV_INSTR.iter().enumerate().map(|x| (*x.1, x.0)).collect();
    pub static ref ALL_BR_INSTR_MAP: HashMap<&'static str, usize> = ALL_BR_INSTR.iter().enumerate().map(|x| (*x.1, x.0)).collect();

    pub static ref RISCV_COFI_MAP: HashMap<&'static str, usize> = RISCV_COFI.iter().enumerate().map(|x| (*x.1, x.0)).collect();

    pub static ref BR_LIST_MAP: HashMap<&'static str, usize> = BR_LIST.iter().enumerate().map(|x| (*x.1, x.0)).collect();
    pub static ref C_BR_LIST_MAP: HashMap<&'static str, usize> = C_BR_LIST.iter().enumerate().map(|x| (*x.1, x.0)).collect();
    pub static ref BR_OFFSET_LIST_MAP: HashMap<i64, usize> = BR_OFFSET_LIST.iter().enumerate().map(|x| (*x.1, x.0)).collect();
    pub static ref C_BR_OFFSET_LIST_MAP: HashMap<i64, usize> = C_BR_OFFSET_LIST.iter().enumerate().map(|x| (*x.1, x.0)).collect();

    /*
    static ref RISCV_OBJDUMP_2_SPIKE_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        m.insert("lnop", "nop");

        m
    };

    static ref RISCV_SPIKE_2_OBJDUMP_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();

        m.insert("c.add", "add");
        m.insert("c.addi", "addi");
        m.insert("c.addi16sp", "addi");
        m.insert("c.addi4spn", "addi");
        m.insert("c.addiw", "addiw");
        m.insert("c.addw", "addw");
        m.insert("c.and", "and");
        m.insert("c.andi", "andi");
        m.insert("c.beqz", "beqz");
        m.insert("c.bnez", "bnez");
        m.insert("c.ebreak", "ebreak");
        m.insert("c.j", "j");

        m.insert("c.jalr", "jalr");
        m.insert("c.jr", "jr");
        m.insert("c.ld", "ld");
        m.insert("c.ldsp", "ld");
        m.insert("c.li", "li");

        m.insert("c.lui", "lui");
        m.insert("c.lw", "lw");
        m.insert("c.lwsp", "lw");
        m.insert("c.mv", "mv");
        m.insert("c.nop", "nop");

        m.insert("c.or", "or");
        m.insert("c.sd", "sd");
        m.insert("c.sdsp", "sd");


        m.insert("c.sdsp", "sd");
        m.insert("c.slli", "slli");
        m.insert("c.srai", "srai");
        m.insert("c.srli", "srli");
        m.insert("c.sub", "sub");
        m.insert("c.subw", "subw");
        m.insert("c.sw", "sw");
        m.insert("c.swsp", "sw");
        m.insert("c.unimp", "unimp");
        m.insert("c.xor", "xor");

        m
    };
    */
}

pub fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

const HI: i64 = 4096;
const LO: i64 = -4096;

const C_HI: i64 = 254; // TODO: check me -> is this value correct?
const C_LO: i64 = -256;

/* ============= make_branch_offset metric ============= */
pub const BR_LIST: &'static [&'static str] = &[
    "beq", "bne", "blt", "bge", "bltu", "bgeu",
    ];

pub const C_BR_LIST: &'static [&'static str] = &[
    "c.beqz", "c.bnez"
    ];

pub const ALL_BR_INSTR: &'static [&'static str] = &[
    "beq", "bne", "blt", "bge", "bltu", "bgeu", "c.beqz", "c.bnez"
    ];

pub const BR_OFFSET_LIST: &'static [i64] = &[
    0, -2, -4, -6, -8, 2, 4, 6, 8, 
    HI, HI-2, HI-4, HI-6, HI-8, LO, LO+2, LO+4, LO+6, LO+8
];

pub const C_BR_OFFSET_LIST: &'static [i64] = &[
    0, -2, -4, -6, -8, 2, 4, 6, 8, 
    C_HI, C_HI-2, C_HI-4, C_HI-6, C_HI-8, C_LO, C_LO+2, C_LO+4, C_LO+6, C_LO+8
];


pub const ALL_RISCV_INSTR_LIST_SIZE: usize = ALL_RISCV_INSTR.len();
pub const BR_OFFSET_LIST_SIZE: usize = ALL_BR_INSTR.len();
pub const BR_OFFSET_RESULT_BUF_SIZE: usize = BR_OFFSET_LIST.len()+2;

pub const MAX_STATES: usize = 3;

