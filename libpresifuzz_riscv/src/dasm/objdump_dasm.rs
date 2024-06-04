// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use std::process::{Stdio, Command};

use crate::elf::ELF;
use libafl::Error;
use libafl_bolts::ErrorBacktrace;
use tempfile::NamedTempFile;
use super::{helper::process_branch, Dasm, DasmInstruction, InstructionType, RiscvInstruction, RiscvInstructions};

#[derive(Debug)]
pub struct ObjdumpDasm {
    elf: ELF,
}

impl ObjdumpDasm {

    pub fn new(template_file: &str) -> Result<Self,Error> {

        let elf = ELF::new(template_file)?;

        Ok(ObjdumpDasm {
            elf: elf,
        })
    }

    fn craft_elf_file(&mut self, output_file: &str, input: &RiscvInstructions) -> Result<(),Error> {
        self.elf.update_ins(input.clone());
        self.elf.write_elf(output_file)?;
        //self.elf.write_elf("/tmp/debug.elf")?;

        Ok(())
    }

    /* Spike uses a small subset of pseudo instructions. This function translates common patterns into the representation used by Spike. */
    fn translate_format(mnemonic: &str, arg: Option<&str>) -> (Option<String>, Option<String>) {

        match (mnemonic, arg) {

            /* unsupported RISCV features (RV32F & RV64F - single-precision floating point instructions) */
            ("flw", _) | ("fsw", _) | ("fmadd.s", _) | ("fmsub.s", _) 
                | ("fnmsub.s", _) | ("fnmadd.s", _) | ("fadd.s", _) 
                | ("fsub.s", _) | ("fmul.s", _) | ("fdiv.s" | "fsqrt.s", _) 
                | ("fsgnj.s", _) | ("fsgnjn.s", _) | ("fsgnjx.s", _) 
                | ("fmin.s", _) | ("fmax.s", _) | ("fcvt.w.s", _) 
                | ("fcvt.wu.s", _) | ("fmv.x.w", _) | ("feq.s", _) | ("flt.s", _) 
                | ("fle.s", _) | ("fclass.s", _) | ("fcvt.s.w", _) | ("fcvt.s.wu", _) 
                | ("fmv.w.x", _) | ("fcvt.l.s", _) | ("fcvt.lu.s", _) | ("fcvt.s.l", _) 
                | ("c.fld", _) | ("c.fldsp", _) | ("c.fsd", _) | ("c.fsdsp", _)
                | ("fcvt.d.lu", _) => (Some("unknown".to_string()), None),

            /* unsupported RISCV features (RV32D & RV64D - double-precision floating point instructions) */
            ("fld", _) | ("fsd", _) | ("fmadd.d", _) | ("fmsub.d", _) | ("fnmsub.d", _) 
                | ("fnmadd.d", _) | ("fadd.d", _) | ("fsub.d", _) | ("fmul.d", _) 
                | ("fdiv.d", _) | ("fsqrt.d", _) | ("fsgnj.d", _) | ("fsgnjn.d", _) 
                | ("fsgnjx.d", _) | ("fmin.d", _) | ("fmax.d", _) | ("fcvt.s.d", _) 
                | ("fcvt.d.s", _) | ("feq.d", _) | ("flt.d", _) | ("fle.d", _) | ("fclass.d", _) 
                | ("fcvt.w.d", _) | ("fcvt.wu.d", _) | ("fcvt.d.w", _) | ("fcvt.d.wu", _) 
                | ("fcvt.l.d", _) | ("fcvt.lu.d", _) | ("fmv.x.d", _) | ("fcvt.d.l", _) 
                | ("fmv.d.x", _) => (Some("unknown".to_string()), None),

            // todo add info on instruction size
            (".2byte", _) => (Some("unknown".to_string()), None),
            (".4byte", _) => (Some("unknown".to_string()), None),
            (".8byte", _) => (Some("unknown".to_string()), None),

            ("c.addi", Some("x0,0")) => (Some("c.nop".to_string()), None),
            ("lnop", None) => (Some("nop".to_string()), None),

            ("sltu", Some(x)) => {
                if x.contains(",x0,") {
                    (Some("snez".to_string()), Some(x.replace(",x0,", "").to_string()))
                }   
                else {
                    (None,None)
                }
            },

            ("addi", Some(x)) => {
                if x.contains(",x0,") {
                    (Some("li".to_string()), Some(x.replace(",x0,", "").to_string()))
                }   
                else {
                    (None,None)
                }
            },

            ("jalr", Some(x)) => {
                if x.starts_with("x0,") {
                    (Some("jr".to_string()), Some(x[2..].to_string()))
                }   
                else {
                    (None,None)
                }
            },

            ("jal", Some(x)) => {
                if x.starts_with("x0,") {
                    (Some("j".to_string()), Some(x[2..].to_string()))
                }   
                else {
                    (None,None)
                }
            },

            _ => (None,None)
        }
    }

    /* helper function to translate args into spike's format (incomplete) */
    fn format_args(args: &String) -> String {

        let comment_pos = match args.find(" #"){
            Some(x) => x,
            None => args.len(),
        };

        let parts: Vec<String> = args[..comment_pos].split(",").map(str::to_string).collect();

        let mut ret = String::new();
        let mut tmp;
        let len = parts.len();
        for (i,e) in parts.iter().enumerate() {

            ret.push_str(match &e.trim() as &str {
                "x1" => "ra",
                "x2" => "sp",
                "x3" => "gp",
                "x4" => "tp",
                "gp0" => "x30",
                "" => continue,
                e => {
                
                    if e.contains(" ") {
                        &e
                    }
                    else {
                        if e.starts_with("0x") {
                            match e[2..].parse::<u64>(){
                                Ok(x) => { 
                                    tmp = x.to_string();
                                    &tmp
                                },
                                Err(_) => &e,
                            }
                        }
                        else {
                            &e
                        }
                    }                
                },
            });

            if i != len-1 {
                ret.push_str(", ");
            }
        }

        return ret;
    }

    pub fn process_elf(elf_file_path: &str, base_address: u64, end_address: u64) -> Result<Vec<(u64, DasmInstruction)>, Error> {
        let mut ret = Vec::<(u64, DasmInstruction)>::new();

        let yaml_fd = std::fs::File::open("./config.yml").unwrap();
        let config: serde_yaml::Value = serde_yaml::from_reader(yaml_fd).unwrap();

        let machine = config["objdump"]["machine"]
            .as_str()
            .unwrap_or("");

        if let Ok(child) = Command::new("riscv64-unknown-elf-objdump")
            .arg("-S")
            .arg(format!("--start-address=0x{:x}", base_address))
            //.arg("-b")
            //.arg("binary")
            .arg("-m")
            .arg("riscv")
            .arg(format!("-M{}", machine))
            .arg("-Mno-aliases")
            .arg("-Mnumeric")
            //.arg("-S")
            //.arg(format!("--start-address={:x}", self.elf.get_payload_address()))
            .arg("-z")
            .arg("-D")
            //.arg("/dev/stdin")
            .arg(elf_file_path)
            .stdout(Stdio::piped())
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .spawn() {

            /* 
                let arg_bytes = self.elf.get_elf_data().unwrap();
                if let Some(child_stdin) = child.stdin.as_mut() {
                    child_stdin.write_all(&arg_bytes).unwrap_or_default();
                }
            */

            if let Some(output_result) = child.wait_with_output().ok() {

                let stdout_data = String::from_utf8_lossy(&output_result.stdout);
                //let stderr_data = String::from_utf8_lossy(&output_result.stderr);

                //println!("stdout: {}", stdout_data);
                //println!("stderr: {}", stderr_data);

                //let mut payload_assembly_found = true;

                for line in stdout_data.lines() {

                    //println!("Line -> {}", line);

                    /* make this optional */
                    //if line.contains("ebreak") {
                    //    break;
                    //}

                    /*
                    if !payload_assembly_found {
                        if line.ends_with("<payload>:") {
                            payload_assembly_found = true;
                            continue;
                        }
                        continue;
                    }
                    */

                    //if payload_assembly_found {
                        let parts: Vec<String> = line.split("\t").map(str::to_string).collect();

                        if parts.len() <= 2 {
                            continue;
                        }

                        /* 
                        println!("0 -> {}", parts[0]);
                        println!("1 -> {}", parts[1]);
                        println!("2 -> {}", parts[2]);
                        println!("3 -> {}", parts[3]);
                        */

                        let address: u64 = u64::from_str_radix(&parts[0][0..parts[0].len()-1].trim(), 16).unwrap();

                        if address >= end_address {
                            break;
                        }

                        if parts[2].starts_with(".8byte") {

                            let bytes8 = u64::from_str_radix(&parts[1].trim().replace(" ", ""), 16).unwrap();

                            ret.push((address, DasmInstruction {
                                mnemonic: "unknown".to_string(),
                                args: None,
                                bytes: bytes8,
                                size: 8,
                                ins_type: InstructionType::Normal,

                            }));

                            continue;
                        }
                        else if parts[2].starts_with(".byte") {
                            let byte6_string = &parts[1].trim().replace(" ", "");
                            
                            //println!("byte6 -> {} ({})\n", byte6_string, byte6_string.len());
                            
                            let bytes6 = u64::from_str_radix(byte6_string, 16).unwrap();


                            ret.push((address, DasmInstruction {
                                mnemonic: "unknown".to_string(),
                                args: None,
                                bytes: bytes6,
                                size: (byte6_string.len()/2) as u8,
                                ins_type: InstructionType::Normal,

                            }));

                            continue;
                        }
                        

                        let (mut mnemonic, mut args) = if parts.len() > 3 {
                            (parts[2].to_string(), Some(parts[3].trim().to_string()))
                        }
                        else {
                            (parts[2].to_string(), None)
                        };

                        (mnemonic, args) = match (mnemonic, args) {

                            (x, Some(y)) => {
                                let (a,b) = Self::translate_format(&x, Some(&y));
                                match a {
                                    Some(x) => (x,b),
                                    None => (x, Some(y)),
                                }
                            },
                            (x, None) => {
                                let (a,b) = Self::translate_format(&x, None);
                                match a {
                                    Some(x) => (x,b),
                                    None => (x, None),
                                }
                            },
                        };


                        let bytes = u32::from_str_radix(&parts[1].trim(), 16).unwrap();
                        let instruction_size = parts[1].trim().len()/2;

                        let args = match args {
                            Some(x) => Some(Self::format_args(&x)),
                            None => None,
                        };

                        let ins_type = process_branch(&mnemonic, args.clone(), address);

                        ret.push((address, DasmInstruction {
                            mnemonic: mnemonic,
                            args: args,
                            bytes: if instruction_size == 2 { bytes & 0xFFFF } else { bytes } as u64,
                            size: instruction_size as u8,
                            ins_type: ins_type,

                        }));
                    //}
                }
            }
        }        

        Ok(ret)
    }

}

impl Dasm for ObjdumpDasm {

    fn process_single(&mut self, ins: &RiscvInstruction, address: u64)  -> Result<DasmInstruction, Error> {

        let mut buffer = RiscvInstructions::new();
        buffer.push(ins.clone());

        let res = self.process_slice(&buffer, address);

        let output = res?;

        if output.len() != 1 {
            println!("--> {:?}", output);
            return Err(Error::Unknown("Failed to disassemble single instruction ".to_string(), ErrorBacktrace::new()))
        }

        return Ok(output[0].1.clone());
    }

    /* The Objdump implementation is pretty ineffiecent compared to Spike as we need to 
     * craft a new ELF file first, load it into objdump and then parse stdout
     * (if possible the spike_dasm implementation should always be preferred).  
     */
    fn process_slice(&mut self, ins: &RiscvInstructions, _address: u64) -> Result<Vec<(u64, DasmInstruction)>, Error> {
        
        let file = NamedTempFile::new()?;
        let tmp_file_path = file.path();
        self.craft_elf_file(&tmp_file_path.to_str().unwrap(), ins)?;

        let base_address = self.elf.get_payload_address();
        let end_address = base_address + ins.len() as u64;

        let ret = Self::process_elf(&tmp_file_path.to_str().unwrap(), base_address, end_address);

        return ret;
    }
}
