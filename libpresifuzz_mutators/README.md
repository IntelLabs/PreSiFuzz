# Command for testing

To run the different tests:
```
cargo test -- --nocapture
```

To disassemble the payload section:
```
riscv64-unknown-elf-objdump -S -D ./testcase.elf | awk -v RS= '/^[[:xdigit:]]+ <payload>/' | head -n 20
```

To change endianess from raw data pinted by the tests:
```
python3
>>> a = [177, 238, 129, 131]
>>> print(hex(int.from_bytes(a, "big")))

```
