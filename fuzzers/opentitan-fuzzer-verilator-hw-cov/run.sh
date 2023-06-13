#!/usr/bin/env bash

# SPDX-FileCopyrightText: 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

rm -rf build

cargo clean \
&& cargo build \
&& rm -rf fusesoc_libraries fusesoc.conf \
&& fusesoc library add opentitan https://github.com/timothytrippel/opentitan.git \
&& cd ./fusesoc_libraries/opentitan \
&& git checkout hwfuzz-checkpoint \
&& pip3 install --user -U -r python-requirements.txt \
&& cd ../.. \
&& cp ./tb/aes_tb.sv ./fusesoc_libraries/opentitan/hw/ip/aes/rtl/aes_tb.sv \
&& cp ./tb/aes_core.sv ./fusesoc_libraries/opentitan/hw/ip/aes/rtl/aes_core.sv \
&& sed -i '/aes.sv/a \ \ \ \ \ \ - rtl\/aes_tb.sv' fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& sed -i "s/toplevel: aes/toplevel: aes_tb/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& sed -i "s/default_tool: icarus/default_tool: verilator/g"  fusesoc_libraries/opentitan/hw/ip/aes/aes.core \
&& fusesoc run --build --flag=fileset_ip --target=syn lowrisc:ip:aes:0.6 --SYNTHESIS --verilator_options="+incdir+$(pwd)/tb -I$(pwd)/tb/include --coverage-toggle --timing --report-unoptflat --cc --exe $(pwd)/tb/src/ot_ip_fuzz_tb.cpp $(pwd)/tb/src/stdin_fuzz_tb.cpp $(pwd)/tb/src/tlul_host_tb.cpp $(pwd)/tb/src/verilator_tb.cpp $(pwd)/tb/src/main.cpp " --make_options "CXXFLAGS=-I$(pwd)/tb/include" \
&& mv build/lowrisc_ip_aes_0.6 build/empty \
&& mv build/empty/syn-verilator/* ./build \
&& ./target/debug/opentitan-fuzzer ./seeds

