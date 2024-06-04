// SPDX-FileCopyrightText: 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
use std::io::Write;

use elf::section::SectionHeader;
use elf::symbol::Symbol;
use elf::{ElfBytes, endian::AnyEndian};
use libafl::Error;
use libafl_bolts::ErrorBacktrace;

use crate::dasm::RiscvInstructions;

#[derive(Debug)]
pub struct ELF{
    payload_address: u64,
    payload_offset: u64,
    payload_size: u64,

    file_data_start: Vec<u8>,
    file_data_payload: RiscvInstructions,

    ebreak_padding: Vec<u8>,
    file_data_end: Vec<u8>,
}

// Abstraction layer to load, generate and modify ELF files
impl ELF {

    pub fn find_symbol(template_file_path: &str, symbol: &str) -> Result<(u64, u64, u64), Error> {
        let path = std::path::PathBuf::from(template_file_path.to_string());
        let data = std::fs::read(path).expect("Could not read file.");
        Self::find_symbol_bytes(&data, symbol)
    }

    /// Returns the address, file offset and size of the payload symbol region as a tuple
    ///
    /// # Arguments
    ///
    /// * `template_file_path` - A string slice that holds the path to the template ELF file
    ///
    pub fn find_symbol_bytes(data: &Vec<u8>, symbol: &str) -> Result<(u64, u64, u64), Error> {
        let slice = data.as_slice();
        let file = ElfBytes::<AnyEndian>::minimal_parse(slice).expect("Open test1");

        let text_shdr: SectionHeader = file
            .section_header_by_name(".text")
            .expect("section table should be parseable")
            .expect("file should have a .note.ABI-tag section");

        let text_section_vaddr = text_shdr.sh_addr;
        let text_section_foffset = text_shdr.sh_offset;
        let text_section_size = text_shdr.sh_size;

        let text_section_end = text_section_vaddr+text_section_size;

        //println!("text_shdr: {:?}", text_shdr);

        let (parsing_table, string_table) = file.symbol_table()
            .expect("Could not parse symtab!")
            .expect("Could not find symtab!");

        let symbol_table: Vec<Symbol> = parsing_table.iter().collect();

        let payload_symbol = 
        symbol_table.iter()                     
            .filter(|s| string_table.get(s.st_name as usize)
            .expect("Could not map symbol_id to string_table entry.") == symbol)
            .next();

        match payload_symbol {
            Some(x) => {
                //println!("=> {:?}", x);
                Ok((
                    x.st_value, 
                    ((x.st_value-text_section_vaddr)+text_section_foffset),
                    (text_section_end-x.st_value)
                ))
            },
            None => {
                Err(Error::Unknown(format!("Could not find <{}> symbol", symbol).to_string(), ErrorBacktrace::new()))
            }
        }
    }

    pub fn first_symbol(template_file_path: &str, section: &str) -> Result<String, Error> {
        let path = std::path::PathBuf::from(template_file_path.to_string());
        let file_data = std::fs::read(path).expect("Could not read file.");
        let slice = file_data.as_slice();
        let file = ElfBytes::<AnyEndian>::minimal_parse(slice).expect("Open test1");


        let (parsing_table, string_table) = file.symbol_table()
            .expect("Could not parse symtab!")
            .expect("Could not find symtab!");

        let text_shdr: SectionHeader = file
            .section_header_by_name(section)
            .expect("section table should be parseable")
            .expect("file should have a .note.ABI-tag section");

        /* 
        let text_section_vaddr = text_shdr.sh_addr;
        let text_section_foffset = text_shdr.sh_offset;
        let text_section_size = text_shdr.sh_size;

        let text_section_end = text_section_vaddr+text_section_size;

        println!("test_shr -> {:#?}", text_shdr);
        */

        if let Some(shdrs) = file.section_headers() {
            //let _: Vec<_> = shdrs.iter().filter(|x| x.sh_name == text_shdr.sh_name).collect();


            let (section_id, _) = shdrs
                                    .iter()
                                    .enumerate()
                                    .filter(|(_,x)| x.sh_name == text_shdr.sh_name && x.sh_addr == text_shdr.sh_addr)
                                    .next().ok_or(Error::Unknown(format!("Could not find section {}", section).to_string(), ErrorBacktrace::new()))?;

            //println!("section_id: {:#?}", section_id);
            //println!("section_obj: {:#?}", section_obj);

            let symbol_table: Vec<Symbol> = parsing_table.iter().collect();

            let mut a: Vec<(u64, String)> = symbol_table.iter()                     
                .filter(|s| (s.st_shndx == section_id as u16))
                .map(|x| (x.st_value, string_table.get(x.st_name as usize).unwrap().to_string()))
                .filter(|(_,y)| y.len() != 0)
                .collect();

            a.sort_by(|x, y| x.0.cmp(&y.0));

            Ok(a.iter().next().ok_or(Error::Unknown(format!("No symbol found for section {}", section).to_string(), ErrorBacktrace::new()))?.1.to_string())
        }
        else {
            Err(Error::Unknown(format!("Could not first symbol in section {}", section).to_string(), ErrorBacktrace::new()))
        }
    }

    fn from_template_bytes(data: &Vec<u8>, _data: Option<&Vec<u8>>) ->  Result<Self, Error>{
        let (payload_address, payload_file_offset, payload_size) = Self::find_symbol_bytes(data, "payload")?;

        let slice = data.as_slice();

        let slice_a = &slice[0..payload_file_offset as usize];
        let slice_b = &slice[payload_file_offset as usize .. (payload_file_offset+payload_size) as usize];
        let slice_c = &slice[(payload_file_offset+payload_size) as usize .. slice.len()];

        let padding16: Vec<u16> = vec![0x9002; payload_size as usize];
        let padding8 = unsafe {
            padding16.align_to::<u8>().1.to_owned()
        }; 

        assert_eq!(payload_size as usize, slice_b.len());

        let file_data_payload = RiscvInstructions::new();

        Ok(Self{
            payload_address: payload_address,
            payload_offset: payload_file_offset,
            payload_size: payload_size,
        
            file_data_start: slice_a.into(),
            file_data_payload: file_data_payload,
            ebreak_padding: padding8,
            file_data_end: slice_c.into(),
        })
    }


    fn from_template(template_file: &str, _data: Option<&Vec<u8>>) ->  Result<Self, Error>{
        let file_data = std::fs::read(template_file).expect("Could not read file.");
        Self::from_template_bytes(&file_data, _data)
    }

    pub fn new(template_file: &str) ->  Result<Self, Error> {
        Self::from_template(template_file, None)
    }

    pub fn from_bytes(bytes: &[u8]) ->  Result<Self, Error> {
        Self::from_template_bytes(&bytes.to_vec(), None)
    }

    pub fn with_slice(template_file: &str, data: &Vec<u8>) -> Result<Self, Error>{
        Self::from_template(template_file, Some(&data))
    }

    pub fn update_ins(&mut self, input: RiscvInstructions) {
        self.file_data_payload = input;
    }

    pub fn update(&mut self, input: &RiscvInstructions) {
        self.file_data_payload = input.clone();
    }

    pub fn get_payload_address(&self) -> u64 {
        self.payload_address
    }

    pub fn get_payload_size(&self) -> u64 {
        self.payload_size
    }

    pub fn get_payload_offset(&self) -> u64 {
        self.payload_offset
    }

    fn get_payload_buffer(&mut self) -> Vec<u8> {

        let mut output = self.file_data_payload.serialize();
        let buf_len = output.len();

        /* ebreak padding */
        let padding_len = self.payload_size as usize - buf_len;
        let padding_slice = &self.ebreak_padding[0..padding_len];

        output.extend_from_slice(padding_slice);

        //file.write_all(padding_slice).expect("Unable to write to testcase.elf... spike failed to create new testcase!");

        return output;
    }

    pub fn get_elf_data(&mut self) -> Result<Vec<u8>, Error> {
        let mut output = Vec::<u8>::new();

        output.extend_from_slice(self.file_data_start.as_slice());
        output.extend_from_slice(&mut self.get_payload_buffer());
        output.extend_from_slice(self.file_data_end.as_slice());

        Ok(output)
    }
    
    pub fn write_elf(&mut self, output_file: &str) -> Result<(), Error> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(output_file).expect("Unable to write testcase.elf, maybe the file already exists?");
        
        let output_buffer = self.get_elf_data()?;

        file.write_all(&output_buffer).expect("Unable to write to elf file ...");
        file.sync_all().expect("Unable to write to testcase.elf... spike failed to create new testcase!");

        Ok(())
    }

}

