FROM paritytech/ci-linux:production as build

WORKDIR /code
COPY . .
# the latest release of ahash uses build_hasher_simple_hash_one 
# which was stabilized in 1.71 but the latest rustc version 
# from the paritytech/ci-linux:production is 1.68.0.
RUN cargo +nightly update -p ahash@0.8.7 --precise 0.8.6
RUN cargo +nightly build --release

FROM ubuntu:22.04
WORKDIR /node

# Copy the node binary.
COPY --from=build /code/target/release/node .

# Install root certs, see: https://github.com/paritytech/substrate/issues/9984
RUN apt update && \
    apt install -y ca-certificates && \
    update-ca-certificates && \
    apt remove ca-certificates -y && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 9944
# Exposing unsafe RPC methods is needed for testing but should not be done in
# production.
#CMD [ "./node-template", "--dev", "--ws-external", "--rpc-external"]
ENTRYPOINT ["./node"]