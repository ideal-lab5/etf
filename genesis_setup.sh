#!/bin/bash
###################
# Generate keys and shares for ETF network genesis state
#################
for i in {1..3}; do
   ./target/debug/node etf generate --output tmp/keys.$i
   ./target/debug/node etf inspect --keys tmp/keys.$i --output tmp/pk.$i
done

# Loop through each file and append its contents to pk followed by a newline
for i in {1..3}; do
    cat tmp/pk.$i >> tmp/pk
    echo "" >> tmp/pk
done


# for i in {1..3}; do
./target/debug/node etf init --committee tmp/pk --shares tmp/shares --params tmp/params --seed 75
# done
