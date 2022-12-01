<!--
SPDX-FileCopyrightText: 2022 Intel Corporation

SPDX-License-Identifier: Apache-2.0
-->

# Disclaimer
All components are provided for research and validation purposes only. Use at your own risk.

# Pre-Silicon Hardware Fuzzing Toolkit
From CPU to GPU, and IPU, the complexity of digital hardware design is
increasing rapidly. This makes it more difficult to verify and/or test.
However, detecting bugs before the hardware design is manufactured is a serious
concern. This is because silicon chips often have no upgrade capability, making
bugs persistent. In this repository, we provide building blocks to apply 
advanced software testing techniques to pre-silicon hardware testing.
These blocks are based on LibAFL, a modern framework for building software
fuzzer.

# Supported OS

This tool has only been tested on Linux based OS, and especially Ubuncu 20.04 LTS.

# Dependencies

This framework relies on the VCS simulator to simulate hardware design and
VERDI to extract coverage information. Please, refer to the official
documentation to install the tool. Please, note that some of these tools may
require specific license scheme.

# Installation

This library is mostly designed around the RUST language. 
For this reson, the initial step is to install 'Cargo'. 
This can be easily done with the following command:
```
curl https://sh.rustup.rs -sSf | sh
```

Then, let's clone and build this tool: 
```
git clone https://github.com/IntelLabs/PreSiFuzz PreSiFuzz
cd PreSiFuzz

git submodule update --init

cargo build
```

# Fuzzing Example

To start playing with the tool, the secworks example is a good candidate.
You can quickly get it running using the following commands:
```
cd fuzzers/baby-rtl-fuzzer
bash ./init.sh
```

# Example targets

The target directory contains examples of design to demonstrate the approach.

* [OpenTitan](/doc/opentitan.md)

