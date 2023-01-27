<!--
SPDX-FileCopyrightText: 2022 Intel Corporation

SPDX-License-Identifier: Apache-2.0
-->

# VCS and LIBVERDI

In this section, we provides more details on the Synopsys based simulation setup.

PreSiFuzz supports VCS based simulation with coverage guided fuzzing.  Synopsys
VCS is one industry leading solution for simulating hardware RTL written in
either VHDL, SystemVerilog or Verilog. VCS offers at least 6 coverate metrics:
line, FSM, condition, toggle, branch, and assertions. By default the coverage is
off. To enable coverage some additional arguments are required to be passed
during the compilation and when starting the simulator.

When compiling add the following parameters:
```
vcs -cm line+fsm+cond+tgl+branch -cm_dir Coverage.vdb 
```

When executing a simv (executable produced by vcs compilation flow), add the following parameters:
```
simv -cm tgl
```

VCS compilation flow produces a simv executable next to some dependencies, a
crsr, Coverage.vdb, and daidir folder. All are required for the simulator to
run properly.  Please, note that vcs uses hardcoded path and so changing the
csrc, simv or daidir names/paths could lead to some 'file not found exception'.
Please, refer to the official documentation for more details (some parameters
may enable to customize dependencies location).

The Coverage.vdb is important as it contains the coverage + symbols that we
need to compute the coverage score. Usually, VCS only admit one vdb per
simulation run. However, a recent feature enable to share the vdb accross
different simulation runs. The Coverage.vdb will then contain the union of all
the previous coverage data.

The following command makes the Coverage.vdb shareable.
```
crg -dir ./output/Coverage.vdb -shared init
```

We usually execute the above steps in a bash script named run.sh.  While
running, the fuzzer needs to parse the vdb structure after each simulation to
indentify inputs of interest. This is the goal of libVerdi, it extracts the vdb
content and create a map the fuzzer can read to identify new seeds. Verdi is a
component from VCS with a couple of debugging features. Among these features,
the libNPI.so (Native Programming Interface) enables users to parse
the vdb structure.  It is writtent in C++, and thanks to a custom C
binding works well with Rust. The source code of this intermediate
layer is stored in the C file 'npi_c.c'. 

The libverdi also contains a libafl based observer with the logic to
interface libverdi and libafl. Please refer to the official
documentation for more details. Note that the fourth parameter of
'update_cov_map' enables user to customized the coverage metric to
parse. 

```
        unsafe {
            let pmap = self.map.as_mut_ptr();
            self.map.set_len(self.cnt);

            let vdb = CString::new(self.vdb.clone()).expect("CString::new failed");
            let db = vdb_cov_init(vdb.as_ptr());

            update_cov_map(db, pmap as *mut c_char, self.cnt as c_uint, 5);

            vdb_cov_end(db);
        }
```

Coverage metrics identifier code:
Toggle -> 5
Line -> 4
