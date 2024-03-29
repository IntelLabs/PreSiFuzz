

git clone https://github.com/googleinterns/hw-fuzzing.git

cargo build

cd hw-fuzzing

export HW_FUZZING=$(pwd) && export PYTHONPATH="./:../:$(pwd)/infra/hwfp:$(pwd)/infra/base-sim/hwfutils"

python3 ./infra/hwfp/hwfp/fuzz.py hw/opentitan/aes/cpp_afl.hjson

cd ..

cp -r ./hw-fuzzing/hw/opentitan/aes/bin ./

cp ./target/debug/opentitan-fuzzer ./

./opentitan-fuzzer ./seeds
