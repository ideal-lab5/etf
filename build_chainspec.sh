# build a dev chainspec
./target/release/node build-spec > etfDevSpec.json
./target/release/node build-spec --chain=etfDevSpec.json --raw > etfDevSpecRaw.json