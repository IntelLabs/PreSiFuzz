#!/usr/bin/env python3

from constants import *
import re
import glob
import os
import pprint
import logging
import collections
import yaml
import sys

pp = pprint.PrettyPrinter(indent=2)
logging.basicConfig(level=logging.INFO, format='%(levelname)s:: %(message)s')

def process_enc_line(line, ext):
    '''
    This function processes each line of the encoding files (rv*). As part of
    the processing, the function eusures that the encoding is legal through the
    following checks::

        - there is no over specification (same bits assigned different values)
        - there is no under specification (some bits not assigned values)
        - bit ranges are in the format hi..lo=val where hi > lo
        - value assigned is representable in the bit range
        - also checks that the mapping of arguments of an instruction exists in
          arg_lut.

    If the above checks pass, then the function returns a tuple of the name and
    a dictionary containing basic information of the instruction which includes:
        - variables: list of arguments used by the instruction whose mapping
          exists in the arg_lut dictionary
        - encoding: this contains the 32-bit encoding of the instruction where
          '-' is used to represent position of arguments and 1/0 is used to
          reprsent the static encoding of the bits
        - extension: this field contains the rv* filename from which this
          instruction was included
        - match: hex value representing the bits that need to match to detect
          this instruction
        - mask: hex value representin the bits that need to be masked to extract
          the value required for matching.
    '''
    single_dict = {}

    # fill all bits with don't care. we use '-' to represent don't care
    # TODO: hardcoded for 32-bits.
    encoding = ['-'] * 32

    # get the name of instruction by splitting based on the first space
    [name, remaining] = line.split(' ', 1)

    # replace dots with underscores as dot doesn't work with C/Sverilog, etc
    name = name.replace('.', '_')

    # remove leading whitespaces
    remaining = remaining.lstrip()

    # check each field for it's length and overlapping bits
    # ex: 1..0=5 will result in an error --> x<y
    # ex: 5..0=0 2..1=2 --> overlapping bits
    for (s2, s1, entry) in fixed_ranges.findall(remaining):
        msb = int(s2)
        lsb = int(s1)

        # check msb < lsb
        if msb < lsb:
            logging.error(
                f'{line.split(" ")[0]:<10} has position {msb} less than position {lsb} in it\'s encoding'
            )
            raise SystemExit(1)

        # illegal value assigned as per bit width
        entry_value = int(entry, 0)
        if entry_value >= (1 << (msb - lsb + 1)):
            logging.error(
                f'{line.split(" ")[0]:<10} has an illegal value {entry_value} assigned as per the bit width {msb - lsb}'
            )
            raise SystemExit(1)

        for ind in range(lsb, msb + 1):
            # overlapping bits
            if encoding[31 - ind] != '-':
                logging.error(
                    f'{line.split(" ")[0]:<10} has {ind} bit overlapping in it\'s opcodes'
                )
                raise SystemExit(1)
            bit = str((entry_value >> (ind - lsb)) & 1)
            encoding[31 - ind] = bit

    # extract bit pattern assignments of the form hi..lo=val
    remaining = fixed_ranges.sub(' ', remaining)

    # do the same as above but for <lsb>=<val> pattern. single_fixed is a regex
    # expression present in constants.py
    for (lsb, value, drop) in single_fixed.findall(remaining):
        lsb = int(lsb, 0)
        value = int(value, 0)
        if encoding[31 - lsb] != '-':
            logging.error(
                f'{line.split(" ")[0]:<10} has {lsb} bit overlapping in it\'s opcodes'
            )
            raise SystemExit(1)
        encoding[31 - lsb] = str(value)

    # convert the list of encodings into a single string for match and mask
    match = "".join(encoding).replace('-','0')
    mask = "".join(encoding).replace('0','1').replace('-','0')

    # check if all args of the instruction are present in arg_lut present in
    # constants.py
    args = single_fixed.sub(' ', remaining).split()
    encoding_args = encoding.copy()
    for a in args:
        if a not in arg_lut:
            logging.error(f' Found variable {a} in instruction {name} whose mapping in arg_lut does not exist')
            raise SystemExit(1)
        else:
            (msb, lsb) = arg_lut[a]
            for ind in range(lsb, msb + 1):
                # overlapping bits
                if encoding_args[31 - ind] != '-':
                    logging.error(f' Found variable {a} in instruction {name} overlapping {encoding_args[31 - ind]} variable in bit {ind}')
                    raise SystemExit(1)
                encoding_args[31 - ind] = a

    # update the fields of the instruction as a dict and return back along with
    # the name of the instruction
    single_dict['encoding'] = "".join(encoding)
    single_dict['variable_fields'] = args
    single_dict['extension'] = [ext.split('/')[-1]]
    single_dict['match']=hex(int(match,2))
    single_dict['mask']=hex(int(mask,2))

    return (name, single_dict)

def same_base_ext (ext_name, ext_name_list):
    type1 = ext_name.split("_")[0]
    for ext_name1 in ext_name_list:
        type2 = ext_name1.split("_")[0]
        # "rv" mean insn for rv32 and rv64
        if (type1 == type2 or
            (type2 == "rv" and (type1 == "rv32" or type1 == "rv64")) or
            (type1 == "rv" and (type2 == "rv32" or type2 == "rv64"))):
            return True
    return False

def create_inst_dict(file_filter, include_pseudo=False, include_pseudo_ops=[]):
    '''
    This function return a dictionary containing all instructions associated
    with an extension defined by the file_filter input. The file_filter input
    needs to be rv* file name with out the 'rv' prefix i.e. '_i', '32_i', etc.

    Each node of the dictionary will correspond to an instruction which again is
    a dictionary. The dictionary contents of each instruction includes:
        - variables: list of arguments used by the instruction whose mapping
          exists in the arg_lut dictionary
        - encoding: this contains the 32-bit encoding of the instruction where
          '-' is used to represent position of arguments and 1/0 is used to
          reprsent the static encoding of the bits
        - extension: this field contains the rv* filename from which this
          instruction was included
        - match: hex value representing the bits that need to match to detect
          this instruction
        - mask: hex value representin the bits that need to be masked to extract
          the value required for matching.

    In order to build this dictionary, the function does 2 passes over the same
    rv<file_filter> file. The first pass is to extract all standard
    instructions. In this pass, all pseudo ops and imported instructions are
    skipped. For each selected line of the file, we call process_enc_line
    function to create the above mentioned dictionary contents of the
    instruction. Checks are performed in this function to ensure that the same
    instruction is not added twice to the overall dictionary.

    In the second pass, this function parses only pseudo_ops. For each pseudo_op
    this function checks if the dependent extension and instruction, both, exit
    before parsing it. The pseudo op is only added to the overall dictionary is
    the dependent instruction is not present in the dictionary, else its
    skipped.


    '''
    opcodes_dir = os.path.dirname(os.path.realpath(__file__))
    instr_dict = {}

    # file_names contains all files to be parsed in the riscv-opcodes directory
    file_names = []
    for fil in file_filter:
        file_names += glob.glob(f'{opcodes_dir}/{fil}')
    file_names.sort(reverse=True)
    # first pass if for standard/regular instructions
    logging.debug('Collecting standard instructions first')
    for f in file_names:
        logging.debug(f'Parsing File: {f} for standard instructions')
        with open(f) as fp:
            lines = (line.rstrip()
                     for line in fp)  # All lines including the blank ones
            lines = list(line for line in lines if line)  # Non-blank lines
            lines = list(
                line for line in lines
                if not line.startswith("#"))  # remove comment lines

        # go through each line of the file
        for line in lines:
            # if the an instruction needs to be imported then go to the
            # respective file and pick the line that has the instruction.
            # The variable 'line' will now point to the new line from the
            # imported file

            # ignore all lines starting with $import and $pseudo
            if '$import' in line or '$pseudo' in line:
                continue
            logging.debug(f'     Processing line: {line}')

            # call process_enc_line to get the data about the current
            # instruction
            (name, single_dict) = process_enc_line(line, f)
            ext_name = f.split("/")[-1]

            # if an instruction has already been added to the filtered
            # instruction dictionary throw an error saying the given
            # instruction is already imported and raise SystemExit
            if name in instr_dict:
                var = instr_dict[name]["extension"]
                if same_base_ext(ext_name, var):
                    # disable same names on the same base extensions
                    err_msg = f'instruction : {name} from '
                    err_msg += f'{ext_name} is already '
                    err_msg += f'added from {var} in same base extensions'
                    logging.error(err_msg)
                    raise SystemExit(1)
                elif instr_dict[name]['encoding'] != single_dict['encoding']:
                    # disable same names with different encodings on different base extensions
                    err_msg = f'instruction : {name} from i'
                    err_msg += f'{ext_name} is already '
                    err_msg += f'added from {var} but each have different encodings in different base extensions'
                    logging.error(err_msg)
                    raise SystemExit(1)
                instr_dict[name]['extension'].extend(single_dict['extension'])
            else:
              for key in instr_dict:
                  item = instr_dict[key]
                  if item["encoding"] == single_dict['encoding'] and same_base_ext(ext_name, item["extension"]):
                      # disable different names with same encodings on the same base extensions
                      err_msg = f'instruction : {name} from '
                      err_msg += f'{ext_name} has the same encoding with instruction {key} '
                      err_msg += f'added from {item["extension"]} in same base extensions'
                      logging.error(err_msg)
                      raise SystemExit(1)

            if name not in instr_dict:
                # update the final dict with the instruction
                instr_dict[name] = single_dict

    # second pass if for pseudo instructions
    logging.debug('Collecting pseudo instructions now')
    for f in file_names:
        logging.debug(f'Parsing File: {f} for pseudo_ops')
        with open(f) as fp:
            lines = (line.rstrip()
                     for line in fp)  # All lines including the blank ones
            lines = list(line for line in lines if line)  # Non-blank lines
            lines = list(
                line for line in lines
                if not line.startswith("#"))  # remove comment lines

        # go through each line of the file
        for line in lines:

            # ignore all lines not starting with $pseudo
            if '$pseudo' not in line:
                continue
            logging.debug(f'     Processing line: {line}')

            # use the regex pseudo_regex from constants.py to find the dependent
            # extension, dependent instruction, the pseudo_op in question and
            # its encoding
            (ext, orig_inst, pseudo_inst, line) = pseudo_regex.findall(line)[0]
            ext_file = f'{opcodes_dir}/{ext}'

            # check if the file of the dependent extension exist. Throw error if
            # it doesn't
            if not os.path.exists(ext_file):
                ext1_file = f'{opcodes_dir}/unratified/{ext}'
                if not os.path.exists(ext1_file):
                    logging.error(f'Pseudo op {pseudo_inst} in {f} depends on {ext} which is not available')
                    raise SystemExit(1)
                else:
                    ext_file = ext1_file

            # check if the dependent instruction exist in the dependent
            # extension. Else throw error.
            found = False
            for oline in open(ext_file):
                if not re.findall(f'^\s*{orig_inst}\s+',oline):
                    continue
                else:
                    found = True
                    break
            if not found:
                logging.error(f'Orig instruction {orig_inst} not found in {ext}. Required by pseudo_op {pseudo_inst} present in {f}')
                raise SystemExit(1)


            (name, single_dict) = process_enc_line(pseudo_inst + ' ' + line, f)
            # add the pseudo_op to the dictionary only if the original
            # instruction is not already in the dictionary.
            if orig_inst.replace('.','_') not in instr_dict \
                    or include_pseudo \
                    or name in include_pseudo_ops:

                # update the final dict with the instruction
                if name not in instr_dict:
                    instr_dict[name] = single_dict
                    logging.debug(f'        including pseudo_ops:{name}')
            else:
                logging.debug(f'        Skipping pseudo_op {pseudo_inst} since original instruction {orig_inst} already selected in list')

    # third pass if for imported instructions
    logging.debug('Collecting imported instructions')
    for f in file_names:
        logging.debug(f'Parsing File: {f} for imported ops')
        with open(f) as fp:
            lines = (line.rstrip()
                     for line in fp)  # All lines including the blank ones
            lines = list(line for line in lines if line)  # Non-blank lines
            lines = list(
                line for line in lines
                if not line.startswith("#"))  # remove comment lines

        # go through each line of the file
        for line in lines:
            # if the an instruction needs to be imported then go to the
            # respective file and pick the line that has the instruction.
            # The variable 'line' will now point to the new line from the
            # imported file

            # ignore all lines starting with $import and $pseudo
            if '$import' not in line :
                continue
            logging.debug(f'     Processing line: {line}')

            (import_ext, reg_instr) = imported_regex.findall(line)[0]
            import_ext_file = f'{opcodes_dir}/{import_ext}'

            # check if the file of the dependent extension exist. Throw error if
            # it doesn't
            if not os.path.exists(import_ext_file):
                ext1_file = f'{opcodes_dir}/unratified/{import_ext}'
                if not os.path.exists(ext1_file):
                    logging.error(f'Instruction {reg_instr} in {f} cannot be imported from {import_ext}')
                    raise SystemExit(1)
                else:
                    ext_file = ext1_file
            else:
                ext_file = import_ext_file

            # check if the dependent instruction exist in the dependent
            # extension. Else throw error.
            found = False
            for oline in open(ext_file):
                if not re.findall(f'^\s*{reg_instr}\s+',oline):
                    continue
                else:
                    found = True
                    break
            if not found:
                logging.error(f'imported instruction {reg_instr} not found in {ext_file}. Required by {line} present in {f}')
                logging.error(f'Note: you cannot import pseudo/imported ops.')
                raise SystemExit(1)

            # call process_enc_line to get the data about the current
            # instruction
            (name, single_dict) = process_enc_line(oline, f)

            # if an instruction has already been added to the filtered
            # instruction dictionary throw an error saying the given
            # instruction is already imported and raise SystemExit
            if name in instr_dict:
                var = instr_dict[name]["extension"]
                if instr_dict[name]['encoding'] != single_dict['encoding']:
                    err_msg = f'imported instruction : {name} in '
                    err_msg += f'{f.split("/")[-1]} is already '
                    err_msg += f'added from {var} but each have different encodings for the same instruction'
                    logging.error(err_msg)
                    raise SystemExit(1)
                instr_dict[name]['extension'].extend(single_dict['extension'])
            else:
                # update the final dict with the instruction
                instr_dict[name] = single_dict
    return instr_dict

def instr_dict_2_extensions(instr_dict):
    extensions = []
    for item in instr_dict.values():
        if item['extension'][0] not in extensions:
            extensions.append(item['extension'][0])
    return extensions

# SPDX-FileCopyrightText: 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

def make_rust(instr_dict):

    header = "// SPDX-FileCopyrightText: 2022 Intel Corporation\n"
    header += "//\n"
    header += "// SPDX-License-Identifier: Apache-2.0\n\n"

    header += "/*\n"
    header += "* This file was automatically generated by PreSiFuzz using riscv-opcodes\n"
    header += "* Please, do not change this file directly but instead look at presifuzz/riscv-opcodes\n"
    header += "* This file contains helper functions to assemble all supported riscv instructions\n"
    header += "*/\n"

    header += "use libafl::prelude::HasRand;\n"
    header += "use crate::instruction::Instruction;\n"
    header += "use libafl::prelude::HasMaxSize;\n"
    header += "use libafl::prelude::MutationResult;\n"
    header += "use libafl_bolts::Named;\n"
    header += "\n"
    header += "use libafl_bolts::{\n"
    header += "    rands::Rand,\n"
    header += "};\n\n"
    
    header += "pub fn build_operand(value: u32, lsb: u32, msb: u32) -> u32 {\n"
    header += "    let nb_bits = msb-lsb + 1;\n"
    header += "    let op_mask = ((1<<(1-1))-1)^((1<<nb_bits)-1);\n"
    header += "    return (value & op_mask) << lsb;\n"
    header += "}\n\n"

    header += "pub fn random_gp_reg<S>(state: &mut S) -> u32\n"
    header += "where\n"
    header += "    S: HasRand,\n"
    header += "{\n"
    header += "    return state.rand_mut().choose(1..32);\n"
    header += "}\n"
    header += "\n"
    header += "pub fn random_big_reg<S>(state: &mut S) -> u32\n"
    header += "where\n"
    header += "    S: HasRand,\n"
    header += "{\n"
    header += "    return state.rand_mut().choose(1..12);\n"
    header += "}\n"


    insn_metadata = "use lazy_static::lazy_static;\n"
    insn_metadata += "lazy_static! {\n"
    insn_metadata += "// Global Vec to hold instructions\n"
    insn_metadata += "pub static ref INSTRUCTIONS: Vec<Instruction> = {\n"
    insn_metadata += "    vec![\n"


    helper_functions = "/* Start of the helper function section */\n" 
    for i in instr_dict:
        #  mask_match_str += f'(, , \"{instr_dict[i]["encoding"].replace("-","?")}\", {mask_bits_array}, 1, ),\n'

        mnemonic = f'{i.upper().replace(".","_")}'
        mask = f'{(instr_dict[i]["match"])}'
        match = f'{(instr_dict[i]["mask"])}'

        operand_builder_block = ""
        for operand in instr_dict[i]["variable_fields"]:
            lsb = 0
            msb = 0
            for name, rng in arg_lut.items():
                if name == operand:
                    lsb = rng[1]
                    msb   = rng[0]
            operand_builder_block += f"    inst |= build_operand({operand}, {lsb}, {msb});\n"
        
        length = 2 if (int(mask, 16) & 0x03) < 0x03 else 4 ;  

        args = ""
        for operand in instr_dict[i]["variable_fields"]:
            args += f"{operand}: u32,"
        args = args[0:-1]

        extension = instr_dict[i]['extension'][0]
        
        variable_fields_array = ""
        for operand in instr_dict[i]["variable_fields"]:
            (msb, lsb) = arg_lut[operand]
            variable_fields_array += f"({lsb},{msb}), "

        helper_functions += f"pub fn {mnemonic.lower()}({args}) -> Instruction\n"
        helper_functions +=  "{\n"
        helper_functions +=  "    let mut inst : u32 = 0;\n"
        helper_functions += f"    let mask: u32 = {mask};\n"
        helper_functions +=  "    inst |= mask;\n"
        helper_functions += f"{operand_builder_block}\n"
        helper_functions +=  "    return Instruction{\n"
        helper_functions += f"        instruction: inst as u64,\n"
        helper_functions += f"        length: {length},\n"
        helper_functions += f"        mask: {mask},\n"
        helper_functions += f"        mmatch: {match},\n"
        helper_functions += f"        extension: String::from(\"{extension}\"),\n"
        helper_functions += f"        mnemonic: String::from(\"{mnemonic.lower()}\"),\n"
        helper_functions += f'        operands: vec![{variable_fields_array}],\n'
        helper_functions +=  "    };\n"
        helper_functions +=  "}\n"

        insn_metadata += '        Instruction {\n'
        insn_metadata += f'            instruction: 0 as u64,\n'
        insn_metadata += f'            length: {length},\n'
        insn_metadata += f'            mask: {mask},\n'
        insn_metadata += f'            mmatch: {match},\n'
        insn_metadata += f"            extension: String::from(\"{extension}\"),\n"
        insn_metadata += f'            mnemonic: String::from(\"{mnemonic}\"),\n'
        insn_metadata += f'            operands: vec![{variable_fields_array}],\n'
        insn_metadata += '        },\n'

    insn_metadata += '        ]\n'
    insn_metadata += '    };\n'
    insn_metadata += '}\n'
    
    all_riscv_def = "pub const ALL_RISCV_INSTR: &'static [&'static str] = &[\n"
    k = 0
    for i in instr_dict:
        mnemonic = f'{i.upper().replace(".","_")}'
        all_riscv_def += f"\"{mnemonic.lower()}\","
        k +=1
        if k % 10 == 0:
            all_riscv_def += "\n"
    all_riscv_def += "];\n"

    rust_file = open('inst.rs','w')
    rust_file.write(f'''
{header}
{all_riscv_def}
{helper_functions}
{insn_metadata}
    ''')
    rust_file.close()

def signed(value, width):
  if 0 <= value < (1<<(width-1)):
    return value
  else:
    return value - (1<<width)


if __name__ == "__main__":
    print(f'Running with args : {sys.argv}')

    extensions = sys.argv[1:]
    for i in ['-c','-latex','-chisel','-sverilog','-rust', '-go', '-spinalhdl']:
        if i in extensions:
            extensions.remove(i)
    print(f'Extensions selected : {extensions}')

    #  include_pseudo = False
    #  if "-go" in sys.argv[1:]:
    include_pseudo = True

    instr_dict = create_inst_dict(extensions, include_pseudo)
    with open('instr_dict.yaml', 'w') as outfile:
        yaml.dump(instr_dict, outfile, default_flow_style=False)
    instr_dict = collections.OrderedDict(sorted(instr_dict.items()))

    if '-rust' in sys.argv[1:]:
        make_rust(instr_dict)
        logging.info('inst.rs generated successfully')
