#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

cargo clean

rm -rf build
rm -rf fusesoc*
rm -rf template
rm -rf output
rm -rf backup_*

fusesoc library add sha256 https://github.com/secworks/sha256

cp ./tb_fuzz.sv ./fusesoc_libraries/sha256/src/tb/

echo "  tb_fuzz:" >> fusesoc_libraries/sha256/sha256.core
echo "    <<: *tb" >> fusesoc_libraries/sha256/sha256.core
echo "    toplevel : tb_fuzz" >> fusesoc_libraries/sha256/sha256.core
sed -i '/tb_sha256.v/a \ \ \ \ \ \ - src\/tb\/tb_fuzz.sv' fusesoc_libraries/sha256/sha256.core

fusesoc run --build --target=tb_fuzz --tool=vcs secworks:crypto:sha256 --vcs_options '-LDFLAGS -Wl,--no-as-needed -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog'

cargo build

mkdir template
cp -r build/secworks_crypto_sha256_0/tb_fuzz-vcs/* ./template

export HW_HOME=$(pwd)

mkdir $HW_HOME/seeds/

./target/debug/baby-rtl-fuzzer $HW_HOME/output/secworks_crypto_sha256_0 \
	$HW_HOME/template/ \
	$HW_HOME/seeds/ \
	$HW_HOME/output/Coverage.vdb \
	$HW_HOME/output/ "+TESTCASE=fuzz_input.hex -cm tgl"
