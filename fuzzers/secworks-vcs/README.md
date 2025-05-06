# SecWorks Fuzzing

SecWorks is a collection of hardware implementation for cryptographic functions 
In this example, we use the SHA-256 as a target for our fuzzer.

For more details, look at the official repo [here](https://github.com/secworks/sha256).

# How to?

First, install fusesoc dependency.
```
python3 -m venv .env

.env/bin/pip3 install --upgrade fusesoc

export PATH=$PWD/.env/bin:$PATH 
```

Then, let's download the secwork design, path the testbench (testharness), and build all (design+fuzzer).
Please, make sure ```VERDI_HOME``` and ```VCS_HOME``` environment variable are set and pointing to valid Synopsys toolchain.
```
cargo build
```

You should be able to run a single instance of the fuzzer using:
```
AFL_LAUNCHER_CLIENT=1 ./target/debug/secworks-vcs
```

To get more insight about fuzzer steps:
```
LIBAFL_DEBUG_OUTPUT=1  AFL_LAUNCHER_CLIENT=1 ./target/debug/secworks-vcs
```

```AFL_LAUNCHER_CLIENT``` is a the fuzzer unique id.
Without this environment variable, the fuzzer starts in broker mode collecting stats log into the systemfile to monitor other fuzzer instances.
