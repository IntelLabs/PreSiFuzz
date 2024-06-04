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
for i in {1..10}; do AFL_LAUNCHER_CLIENT=$i ./cva6_vcs_fuzzer ; done
```
