<!--
SPDX-FileCopyrightText: 2022 Intel Corporation

SPDX-License-Identifier: Apache-2.0
-->

# Fuzzing OpenTitan
OpenTitan is an open-source silicon root-of-trust, please refer to [the official website](https://opentitan.org/) for more details.

In the following, we explain how to start fuzzing OpenTitan AES IP using libAFL.
```
cd fuzzers/opentitan-fuzzer-vcs

bash ./run.sh
```
Note: The docker build is deprecated.

The environement variable 'TMPDIR' is used to set the workdir for the different fuzzers. By default `/tmp/presifuzz_*/`
For every seed, the generated VCS files are saved in a dedicated folder whose name starts with 'backup_{id}', and where 'id' is a unique identifier. These directories contain the 'vdb' structures with coverage data. A merged report for all the 'vdb' can be generated using the following command:
```
urg $(find -maxdepth 2 -name "Coverage.vdb" -exec echo "-dir " {} \;) -format both -metric tgl -report urg_report
```

If you prefer getting a report per test:
```
find -maxdepth 2 -name "backup_*" -exec urg -dir {}/Coverage.vdb -format both -metric tgl -report urg_report_{} \;
```

# Credits

This example replicates the work from [Timothy Tripple, et all](https://github.com/googleinterns/hw-fuzzing), except that we use vcs for simulating hardware.
The seeds provided in the 'seeds' comes directly from [this repository](https://github.com/googleinterns/hw-fuzzing).

The RTL code comes from the [OpenTitan team](https://opentitan.org/).
