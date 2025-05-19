# delete if mp.bin or mp.abi exists
if [ -f mp.bin ] || [ -f mp.abi ]; then
    rm mp.bin mp.abi
fi
# compile the contract in the path of the contract directory
solc --bin --abi --optimize --via-ir --output-dir . mp.sol
