# Fuzzer Overview

This documentation outlines the process of setting up and using an RTL fuzzer for CVA6 architecture using libAFL and PreSiFuzz. 
This setup demonstrates feedback-guided fuzzing using hardware code coverage reported by commercial simulator.

# Prerequisites

The following tools need to be installed on your own.

* Latest version of [Spike](https://github.com/riscv-software-src/riscv-isa-sim), the RISC-V ISA simulator, installed. 
Follow the installation instructions from the Spike GitHub repository.

* Synopsys VCS (Verilog Compiler Simulator) installed and properly initialized. VCS is a widely used Verilog simulator.
 Ensure it is configured and ready for simulation tasks.

* [RISC-V GNU Compiler Toolchain] (https://github.com/riscv-collab/riscv-gnu-toolchain)

## Building

The build.rs script performs the following tasks:

* Initially, it downloads the `cva6` mainstream, initializes submodules, and applies the `cva6.patch` patch.
* Next, it downloads the source code for `libfesvr` and builds it.
* Finally, it compiles the `./src/testcase.S` file and builds the simv self-contained simulator. Generated files are then copied into the `build` folder.

To complete the steps above, simply run:
```sh
$ cargo build
```
## Troubleshooting

It may happened that some environement variables are not properly define and the build script may fail.
Please, check `./cva6/verif/simv/setup-env.sh` and make sure that settings are valid.

## Running the fuzzer

When starting, the fuzzer creates a work directory where it saves intermediates files such as mutants, and symbolic links to the `simv` and its dependencies in `./build`.
Work directory are saved into `TMPDIR` with a unique directory per fuzzer instance. Naming follows `presifuzz_<id>`.
Synchronization information are saved into the `sync` directory, it includes `testcase` and associated `coverage map`.

```
$ cp ../../target/debug/cva6_vcs_fuzzer .
$ mkdir sync
```

To run a single fuzzer instance:
```
$ AFL_LAUNCHER_CLIENT=1 ./cva6_vcs_fuzzer
```

To run multiple fuzzer instances:
```
for i in {1..10}; do AFL_LAUNCHER_CLIENT=$i ./cva6_vcs_fuzzer & done
```

# Customizing

The fuzzer is bootstraped using the seed files into the `seeds` folder. Feel free to customize the content of this file with any interesting seed.
When starting the fuzzer loads the initial inputs (i.e., the seeds), and only keep interesting ones in the corpus (i.e., coverage novelty).
Coverage novelty consider any changes for all supported code coverage metrics on vcs, i.e., branch, conditional, line, toggle, and FSM.
Then, starts the fuzzer loop that iteratively calls the different stages. 
StdMutationalStage is responsible for generating new mutant by applying mutation to the existing testcase in the corpus. 
The mutations work at the ISA level by first deserializing the binary testcase into stream of instruction, then different mutations might be applied (e.g., adding instruction, removing instruction, changing opcode, ..). 
The mutation can easily be customized by changing `../../libpresifuzz_mutators/src/riscv_isa.rs`. 
The generated testcase is then inserted into a template ELF file by simplify injecting the code after the `payload` label. 
This template contains epilogue and prologue code. 
The current version is very simple. We first init registers to some known values, and we change the `mtvec` to points to our own trap handler.
The trap handler is there to stop earlier the testcase execution if we trop too often. Otherwise, it always try to return to the instruction after the failing one.
This version is a naive implementation, better performance could be achieved with some changes on the testharness (e.g., early simulation stop, irq support).


# Ploting data

The fuzzer saves statistics into the `sync`directory.
It is possible to plot coverage over time using the `plot.py`:

```python
python3 ./plot.py -m branch -d ./sync
```

The `-m` option is there to provide the coverage metric that is either tgl, cond, branch, line, fsm.
The `-d` points to the directory where stats are saved.