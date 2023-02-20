#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

rm -rf build

cargo clean \
&& LIBRARY_PATH="$VERDI_HOME/share/NPI/lib/linux64/" cargo build \
&& sudo -S ldconfig $VERDI_HOME/share/NPI/lib/LINUX64 \
&& rm -rf fusesoc_libraries fusesoc.conf \
&& fusesoc library add opentitan https://github.com/timothytrippel/opentitan.git \
&& cd ./fusesoc_libraries/opentitan \
&& git checkout hwfuzz-checkpoint \
&& pip3 install --user -U -r python-requirements.txt \
&& cd ../.. \
&& cp ./tb/tb_aes.sv ./fusesoc_libraries/opentitan/hw/ip/aes/rtl/tb_aes.sv \
&& sed -i '/aes.sv/a \ \ \ \ \ \ - rtl\/tb_aes.sv' fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& sed -i "s/toplevel: aes/toplevel: tb_aes/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& sed -i "s/default_tool: icarus/default_tool: vcs/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& fusesoc run --build --flag=fileset_ip --target=syn lowrisc:ip:aes:0.6 --SYNTHESIS  --vcs_options "-LDFLAGS -Wl,--no-as-needed -kdb -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb -full64 -sverilog -ntb_opts uvm-1.2 -debug_access+all -lca" \
&& mv build/lowrisc_ip_aes_0.6 build/empty \
&& mv build/empty/syn-vcs/* ./build \
&& ./target/debug/opentitan-fuzzer

