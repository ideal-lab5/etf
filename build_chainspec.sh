# build a dev chainspec
#cargo build --release
./target/release/node build-spec --dev > etfDevSpec.json
./target/release/node build-spec --chain=etfDevSpec.json --raw > local_chainspec.json
#rm etfDevSpec.json
