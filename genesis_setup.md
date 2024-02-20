# A guide on setting up the network with multiple initial authorities

Suppose we want three intial network authorities.

The goal is to construct a shares for the initial network authority set.

To generate a committee of size 3 (arbitary) run the genesis setup script:

```
chmod +x genesis_setup.sh
./genesis_setup.sh
```

This will output a few files in a new /tmp folder (unless you already have one). The ones you need to look at are the `pk.*` files, the `shares`, and the `acss_params` files. Each one contains data that we need to configure in our chainspec. Firstly, we need to tell the chain spec which authority owns which public key.

1. Assign pk.1 to Alice, pk.2 to Bob, and pk.3 to Charlie by copying the value and modifying the chain_spec to set

``` rust
authority_keys_from_seed("Alice", b"the copied value"), 
```

1. Replace the arguments for initial capsule and acss params with the 'shares' file content and 'acss_params' file content, repsectively.