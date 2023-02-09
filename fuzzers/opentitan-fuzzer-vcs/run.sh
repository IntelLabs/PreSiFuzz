#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

cargo clean
LIBRARY_PATH="/usr/synopsys/verdi/R-2020.12-SP2-11-T-20220930/share/NPI/lib/linux64/" cargo build

rm -rf fusesoc_libraries fusesoc.conf

fusesoc library add opentitan https://github.com/timothytrippel/opentitan.git
cd ./fusesoc_libraries/opentitan
git checkout hwfuzz-checkpoint
cd ../..

cp ./tb/tb_aes.sv ./fusesoc_libraries/opentitan/hw/ip/aes/rtl/tb_aes.sv

sed -i '/aes.sv/a \ \ \ \ \ \ - rtl\/tb_aes.sv' fusesoc_libraries/opentitan/hw/ip/aes/aes.core
sed -i "s/toplevel: aes/toplevel: tb_aes/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core
sed -i "s/default_tool: icarus/default_tool: vcs/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core

fusesoc run --build --flag=fileset_ip --target=syn lowrisc:ip:aes:0.6 --SYNTHESIS  --vcs_options "-LDFLAGS -Wl,--no-as-needed -kdb -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog -ntb_opts uvm-1.2 -debug_access+all -lca"

rm -rf output
mkdir output
cp -r build/lowrisc_ip_aes_0.6/syn-vcs/* ./output
crg -dir ./output/Coverage.vdb -shared init

export HW_HOME=$(pwd)

mkdir $HW_HOME/seeds/

sudo ldconfig /usr/synopsys/verdi/R-2020.12-SP2-11-T-20220930/share/NPI/lib/LINUX64

rm -rf backup_*


./target/debug/opentitan-fuzzer $HW_HOME/output/lowrisc_ip_aes_0.6 \
  $HW_HOME/seeds/ \
  $HW_HOME/output/Coverage.vdb \
  $HW_HOME/output/ "+TESTCASE=fuzz_input.hex -cm tgl"

