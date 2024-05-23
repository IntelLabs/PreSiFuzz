# CVA6 Fuzzer

Welcome to the CVA6 Fuzzer, a tool designed to fuzz the CVA6 CPU implementation, which can be found at [openhwgroup/cva6](https://github.com/openhwgroup/cva6). CVA6 is a CPU that implements a 64-bit RISC-V instruction set architecture.

## Installation

To install the CVA6 Fuzzer, follow these steps:

1. Clone the repository:

   ```bash
   git clone https://github.com/yourusername/cva6-fuzzer.git



# CVA6


To synthetize the CVA6 vcs testbench, it is require to install libfesvre that is included  Spike the riscv isa simulator. 
```bash
export RISCV=/home/$USER/riscv
bash ./ci/install-fesvr.sh
```

```
cd verif/core-v-verif/vendor/riscv/riscv-isa-sim/
mkdir build && cd build
../configure --prefix=$RISCV
make install-config-hdrs install-hdrs libfesvr.so
cp libfesvr.so $RISCV/lib
popd
```

* Note: The `setup-env.sh` script complains if Verilator is not installed, however Verilator is not used by the fuzzer. 
If you do not have it already installed, feel free to comment the corresponding lines at the end of the script. 
```
cd ./verif/sim

source ./setup-env.sh

export DV_SIMULATORS=vcs-testharness
export CVA6_HOME_DIR=$(pwd)/../..

python3 cva6.py --target cv32a60x --iss=$DV_SIMULATORS --iss_yaml=cva6.yaml \
    --c_tests ../tests/custom/hello_world/hello_world.c \
    --linker=../tests/custom/common/test.ld \
    --gcc_opts="-static -mcmodel=medany -fvisibility=hidden -nostdlib \
    -nostartfiles -g \
    ../tests/custom/common/crt.S -lgcc \
    -I../tests/custom/env -I../tests/custom/common"
```

```
cd fuzzers/cva6_vcs_fuzzer
cargo build
```

```
./target/debug/cva6_vcs_fuzzer $CVA6_HOME_DIR/work-vcs/simv +permissive \
  +tohost_addr=80001000 \
  +elf_file=testcase.elf +permissive-off ++testcase.o +debug_disable=1 +ntb_random_seed=1  \
  -sv_lib /home/nasm/riscv/lib/libfesvr
```

The fuzzer runs with a custom RISCV ISA mutator 
