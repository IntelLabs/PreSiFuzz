#!/bin/bash
set -e

if [ ! -d "chipyard" ] ; then
    echo "chipyard is not present in the current directory. Downloading.."

    git clone https://github.com/ucb-bar/chipyard.git
fi

# Clone the chipyard repository and checkout branch 1.11.0
REPO_URL="https://github.com/ucb-bar/chipyard.git"
BRANCH="1.11.0"

if [ ! -d "chipyard" ] ; then
  echo "Cloning the chipyard repository..."
  git clone -b $BRANCH $REPO_URL
else
  echo "failed to clone chipyard.."
fi

cd chipyard

./build-setup.sh --skip-marshal --skip-firesim --skip-toolchain --skip-conda riscv-tools

source ./env.sh

git apply ../chipyard_cov.patch

cd sims/vcs/

make

echo "Build testcase"

RISCV_BENCHMARKS="./toolchains/riscv-tools/riscv-tests/benchmarks"
riscv64-unknown-linux-gnu-gcc -I $RISCV_BENCHMARKS/../env -I $RISCV_BENCHMARKS/common -I./testcase -DPREALLOCATE=1 -mcmodel=medany -static -std=gnu99 -O2 -ffast-math -fno-common -fno-builtin-printf -fno-tree-loop-distribute-patterns -o ../build/iram.elf $RISCV_BENCHMARKS/common/crt.S ../src/testcase.S -static -nostdlib -nostartfiles -lm -lgcc -T $RISCV_BENCHMARKS/common/test.ld

