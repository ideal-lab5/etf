# EtF Pallet

The ETF pallet powers the ETF consensus mechanism, alongside the aura pallet. Specifically, this pallet is designed to hold public parameters required for our protocol as well as act as facilitate the dynamic committee proactive secret sharing scheme. 

## DPSS Integration

This pallet allows network authorities to participate in the dynamic committee proactive secret sharing scheme for ETF consensus. It works as follows:

1. outgoing committees produce new shares (encrypted) offchain and construct an MMR where each leaf is one of the encrypted shares. 
2. They publish the MMR root onchain
3. They send the actual MMR to the offchain index, where it is identified by the root
4. New committee members read the MMR roots from the chain

## Runtime Storage

- `IBEParams`: The publicly known generator required for the IBE block seals.

## Extrinsics

- `update_ibe_params`: Update the IBE public parameter. Only callable by the root node.

## License
GPLv3.0