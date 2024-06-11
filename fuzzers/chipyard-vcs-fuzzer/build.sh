#!/bin/bash
set -e

conda install -n base conda-libmamba-solver
conda config --set solver libmamba
conda install -n base conda-lock==1.4.0
conda activate base
if [ ! -d "chipyard" ] ; then
    git clone https://github.com/ucb-bar/chipyard.git
fi
cd chipyard
git checkout 1.11.0
./build-setup.sh riscv-tools
source ./env.sh
cd sims/vcs/
git apply ../chipyard_cov.patch
make -j 12

echo "Build testcase"
RISCV_BENCHMARKS="./toolchains/riscv-tools/riscv-tests/benchmarks"
riscv64-unknown-linux-gnu-gcc -I $RISCV_BENCHMARKS/../env -I $RISCV_BENCHMARKS/common -I./testcase -DPREALLOCATE=1 -mcmodel=medany -static -std=gnu99 -O2 -ffast-math -fno-common -fno-builtin-printf -fno-tree-loop-distribute-patterns -o ../build/iram.elf $RISCV_BENCHMARKS/common/crt.S ../src/testcase.S -static -nostdlib -nostartfiles -lm -lgcc -T $RISCV_BENCHMARKS/common/test.ld