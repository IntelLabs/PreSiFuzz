use serde::{Deserialize, Serialize};

//////////////////////////////////////////////////////////
///
///
//////////////////////////////////////////////////////////
/// An input for gramatron grammar fuzzing

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Instruction {
    pub instruction: u64, 
    pub length: usize, 
    pub mask: u32, 
    pub mmatch: u32,
    pub mnemonic: String,
    pub extension: String,
    pub operands: Vec<(u32, u32)>,
}

