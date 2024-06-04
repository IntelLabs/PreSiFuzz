use libafl_bolts::{Error, ErrorBacktrace};

use super::InstructionType;


/* Map Ojbdump/Spike instruction args output to its register number (e.g. "ra" to 1; "x1").*/
fn process_register(reg: &str) -> Result<u8, Error> {
    let reg_str = reg.trim();

    match reg_str.chars().nth(0) {
        Some('x') => {
            Ok(u8::from_str_radix(&reg_str[1..], 10).unwrap())
        },
        Some(_) => {
            match reg_str {
                "zero" =>   { Ok(0) },
                "ra" =>     { Ok(1) },
                "sp" =>     { Ok(2) },
                "gp" =>     { Ok(3) },
                "tp" =>     { Ok(4) },
                "t0" =>     { Ok(5) },
                "t1" =>     { Ok(6) },
                "t2" =>     { Ok(7) },
                _ => {
                    Err(Error::Unknown(format!("invalid register ({}) ?!", reg_str), ErrorBacktrace::new()))
                }
            }
        }
        None => {
            Err(Error::Unknown(format!("invalid register ({}) ?!", reg_str), ErrorBacktrace::new()))
        }
    }
}

pub fn reg_to_str_spike(reg: u8) -> String {
    match reg {
        1 => "ra".to_string(),
        2 => "sp".to_string(),
        3 => "gp".to_string(),
        4 => "tp".to_string(),

        _ => format!("x{}", reg).to_string(),
    }
}

fn process_address_format(arg: &str, address: u64) -> i32 {

    let arg = arg.trim();

    //println!("arg '{}'", arg);

    if arg.starts_with("pc ") {
        // spike format (example: c.bnez x11, pc + 112)
        let offset_str: Vec<String> = arg.split(" ").map(str::to_string).collect();

        let offset_value_str = offset_str[2].trim();

        let offset = if offset_value_str.starts_with("0x") {
            i32::from_str_radix(&offset_value_str[2..], 16).unwrap()
        }
        else {
            i32::from_str_radix(offset_value_str, 10).unwrap()
        };

        //let offset = i32::from_str_radix(&offset_str[3].trim(), 10).unwrap(); 

        if arg.chars().nth(3).unwrap() == '+' {
            offset
        }
        else {
            offset * -1
        }
    }
    else {
        // objdump format (example: c.beqz x11, 100204 <payload+0x180>)
        let parts: Vec<String> = arg.split(" ").map(str::to_string).collect();

        let target_address = u32::from_str_radix(&parts[0].trim(), 16).unwrap();

        target_address.wrapping_sub(address as u32) as i32
    }
}

pub fn process_branch(mnemonic: &str, arg: Option<String>, address: u64) -> InstructionType {

    match arg {
        Some(x) => {
            let parts: Vec<String> = x.split(",").map(str::to_string).collect();

            match mnemonic {

                "c.beqz" | "c.bnez" | "beqz" | "bgez" | "bltz" | "bnez" => {
                    //println!("-> {} {}", mnemonic, x);
                    let reg = process_register(&parts[0]).unwrap(); 
                    let offset = process_address_format(&parts[1], address);
    
                    //println!("BRANCH1: mnemonic: {} - R{} / {} PC: {:x}", mnemonic, reg, offset, address);
        
                    InstructionType::CondBranchRelative((reg, offset)) //(address as i32 - pc as i32)))
                },
                
                "blt" | "beq" |  "bge" | "bgeu" | "bne" | "bltu" => {
                    let reg1 = process_register(&parts[0]).unwrap(); 
                    let reg2 = process_register(&parts[1]).unwrap(); 
                    let offset = process_address_format(&parts[2], address);

                    //println!("BRANCH2: mnemonic: {} - R{} R{}/ {:x}", mnemonic, reg1, reg2, address);
    
                    if reg2 == 0 {
                        InstructionType::CondBranchRelative((reg1, offset)) //(address as i32 - pc as i32)))
                    }
                    else {
                        InstructionType::CondBranchRelativeCMP((reg1, reg2, offset)) //(address as i32 - pc as i32)))
                    }

                },

                
                "jal" => {
                    //println!("-> {} {} {}", mnemonic, x, parts.len());

                    let (reg, offset) = if parts.len() == 1 {
                        (1, process_address_format(&parts[0], address)) // ra -> x1
                    }
                    else {
                        (process_register(&parts[0]).unwrap(), process_address_format(&parts[1], address))
                    };

    
                    //println!("JUMP1:  mnemonic: {} - R{} / {:x}", mnemonic, reg, address);
    
                    InstructionType::BranchRelativeStore((reg, offset)) //(address as i32 - pc as i32)))
                },
    
                
                "c.j" | "j" => {
                    //println!("-> {} {} {}", mnemonic, x, parts.len());

                    let offset = process_address_format(&parts[0], address);

                    //let address = u32::from_str_radix(&parts[0], 16).unwrap();
                    
                    //println!("JUMP2:  mnemonic: {} - {:x}", mnemonic, address);
    
                    InstructionType::BranchRelativeStore((0, offset)) //(address as i32 - pc as i32))
                },
                
                /* far transfers */
                "c.jr" | "jr" | "c.jalr" => {
                    //println!("-> {} {}", mnemonic, x);

                    let (reg, offset) = if parts[0].contains("(") && parts[0].contains(")") {
                        let a: Vec<String> = parts[0].split("(").map(str::to_string).collect();
                        let b: Vec<String> = a[1].split(")").map(str::to_string).collect();
                        //println!("a: {:?} b: {:?} ", a, b);
                        (process_register(&b[0]).unwrap(), i32::from_str_radix(&a[0].trim(), 10).unwrap())
                    }
                    else {
                        (process_register(&parts[0]).unwrap(), 0)
                    };
    
                    InstructionType::BranchAbsoluteStore((0, reg, offset))
                }
                
                
                "jalr" => {
                    /*
                        spike   -> jalr sp, x19, -1183
                        objdump -> jalr sp, -1183(x19)
                     */
                    //println!("-> {} {}", mnemonic, x);

                    if parts.len() == 1 {
                        return InstructionType::BranchAbsoluteStore((0, process_register(&parts[0]).unwrap(), 0));
                    }

                    let (reg1, reg2, offset) = if parts[1].contains("(") && parts[1].contains(")") {
                        let a: Vec<String> = parts[1].split("(").map(str::to_string).collect();
                        let b: Vec<String> = a[1].split(")").map(str::to_string).collect();
                        //println!("a: {:?} b: {:?} ", a, b);
                        (process_register(&parts[0]).unwrap(), process_register(&b[0]).unwrap(), i32::from_str_radix(&a[0].trim(), 10).unwrap())
                    }
                    else {
                        //println!("-> {:?}", parts);

                        (process_register(&parts[0].trim()).unwrap(), 
                        process_register(&parts[1].trim()).unwrap(), 
                        i32::from_str_radix(&parts[2].trim(), 10).unwrap())
                    };

                    InstructionType::BranchAbsoluteStore((reg1, reg2, offset))

                }
    
                _ => {
                    InstructionType::Normal
                }
            }
        }

        None => {

            match mnemonic {
                "ret" => {
                    InstructionType::BranchAbsoluteStore((0, 1, 0))
                }
                "c.ebreak" => {
                    InstructionType::BranchAbsoluteStore((0, 0, 0))
                }
                _ => {
                    InstructionType::Normal
                }
            }
        }
    }
}
