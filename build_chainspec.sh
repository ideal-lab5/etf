# build a dev chainspec
#cargo build --release
./target/debug/node build-spec --dev > etfDevSpec.json
./target/debug/node build-spec --chain=etfDevSpec.json --raw > local_chainspec.json
#rm etfDevSpec.json
