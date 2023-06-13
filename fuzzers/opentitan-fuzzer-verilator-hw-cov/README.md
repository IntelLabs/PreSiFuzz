<!--
SPDX-FileCopyrightText: 2022 Intel Corporation

SPDX-License-Identifier: Apache-2.0
-->

# Fuzzing OpenTitan
OpenTitan is an open-source silicon root-of-trust, please refer to [the official website](https://opentitan.org/) for more details.

In the following, we explain how to start fuzzing OpenTitan AES IP using libAFL + Verilator (toggle coverage).
```
cd fuzzers/opentitan-fuzzer-verilator-hw-cov

bash ./run.sh
```
Note: The docker build is deprecated. Instead we use pip and fusesoc to install the requirements.
The script also patches the `aes.core` file to switch from icarus to Verilator, and set `aes_tb` as top module.
However, the Verilator testbench is written in c++ and saved into the `tb` folder.

After each simulation completion, Verilator saves the toggle coverage onto the disk at 'logs/coverage.dat'.
Please, refer to [Verilator-coverage](https://verilator.org/guide/latest/exe_verilator_coverage.html) if you need to aggregate the results.

# Credits

This example replicates the work from [Timothy Tripple, et all](https://github.com/googleinterns/hw-fuzzing), except that we use Verilator hardware coverage as a feedback signal for the fuzzer.
The seeds provided in the 'seeds' comes directly from [this repository](https://github.com/googleinterns/hw-fuzzing).
The testbench provided in the 'tb' comes directly from [this repository](https://github.com/googleinterns/hw-fuzzing).

The RTL code comes from the [OpenTitan team](https://opentitan.org/).


