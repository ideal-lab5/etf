# ETF Finality Gadget

This is a specification of the Encryption to the Future Finality Gadget.

## Motivation

The goal is to be able to provide cross-chain, interoperable, publicly verifiable on-chain randomness as well as timelock encryption capabilities.

- Allow customization of signature schemes to adapt to different target chains (BEEFY).
- we want to be able to provide PVOR across chains that may use varying curves/crypto
- We can enable multiple streams of randomness in this way, where multiple authority sets run the protocol.
- A post-finality gadget allows us to customize the crypto, making it easy to bridge to networks that support different curves.
- We can upgrade the base network's consensus as needed without interfering with randomness production

- why randomness?
    - enables new kinds of protocols for decentralized systems
    - e.g. async coinflipping, trustless asset swaps, etc.
    - enables timelock encryption and practical witness encryption

## Protocol Overview

The goal of the protocol is to produce publicly verifiable randomness. It accomplishes this by using threshold identity based encryption, DLEQ proofs, and dynamic committee proactive secret sharing. The ETF finality gadget follows the GRANDPA finality gadget, with the authority set (normally) being the same for both protocols. Unlike similar consensus mechanisms like BEEFY (which ETF is based on), the goal is not to enable light clients that bridge to the base network but rather that bridge to a chain of publicly verifiable onchain randomness. The output of the ETF finality gadget can be considered as a sidechain.

The idea is that each ETF authority broadcasts the output of the ETF sign function as its 'vote'. 

When the protocol initializes, each authority produces two new random values, which will be used to produce 'shares' of randomness later on. To begin, they each two values and the nproduce a 'resharing' of their secrets with the initial validator set. The resharing is broadcast to all peers along with a Pedersen commitment to the secrets (i.e. public broadcast). We assume that this broadcast is eventually consistent and that each initial validator has received at least a threshold of resharings by the time the next finalized block is ready.

When the ETF authority set encounters a new block, each authority calculates a share of the secret that will be leaked and submits it as a signed 'vote'. When at least a threshold of authorities have submitted votes they can be 'tallied'. This involves interpolations of a polynomial (Lagrange interpolation) and then evaluation of it as 0. The recovered value is the the round secret. Along with this, each validator includes a DLEQ proof. Using BLS12-381 makes it efficient to aggregate and verify these proofs.


Some brainstorming:

I think I want to do the initial 'resharing' as a new GossipMessage type.
As a GossipMessage, authorities can prepare the initial resharings and the send the message to all other ETF authorities. When they recieve the message, they would run the ACSS recover algorithm.

- This would mean the initial resharing is never 'on chain'. Is this good? bad? Idk. It's probably fine.
- I need to be careful that the resharing completes before the ETF protocol is active. This also means that after performing the recovery each validator needs to share a public key, so perhaps ANOTHER gossip mesage for sharing this? RESHARE/RECOVER messages. 
- This would also mean that we need to persist the commitments shared by the validators after they call recover. We probably need to add this to the voter state persisted in the aux db.
->> code in worker.rs


## Definitions

For the following, $H_1: \{0, 1\}^* \to \mathbb{G}_1$ is the hash to G1 function. Assume that it is cryptographically secure.

**Verifiable IBE Extract**

The ETF mechanism works by leaking IBE secrets over time. By "tagging" the output of the IBE extract function with a DLEQ proof, we can verify that a given IBE secret was calculated by the known owner of some share of a secret shared among a committee. In the IBE extract function, an identity's secret is calculated as $d_{ID} = s Q_{ID}$ where $s$ is a secret key and $Q_{ID} = HashToG1(ID)$. To be more specific, we instantiate our IBE scheme using BLS12-381 and type III pairings. Briefly, we can summarize this as the functions:

$(r, \pi) \leftarrow ETF.Sign(d, ID)$ where $ID \in \{0, 1\}^*$ and $d \xleftarrow{R} \mathbb{Z}_p$ is a secret key. 
$0/1 \leftarrow ETF.Verify(r, P, \pi)$ where $(r, \pi)$ is the output of the Sign function and $P$ is the author to verify. It outputs 0 if the proof is invalid (does not show that the secret was calculated by $P$), or 1 otherwise.

The idea is that each member of the validator set calculates an 'ETF' signature for the current block, producing the output $(r,\pi)$ and submitting it as a 'vote' which is broadcast to each peer. Block importers 'tally' each vote by interpolating a round secret. 

**ETF Session key pair**: $(d_i^r, Q_i^r)$
- By default, the protocol uses BLS12-381, so $d_i^r \xleftarrow{R} \mathbb{Z}_p$ and $Q_i^r = d_i^r P$. In the future we plan to support other curves as well. 

**Dyamic Committee Proactive Secret Sharing**

TODO

A **commitment**, $\mathcal{C}$, contains the output of the ETF `Sign` function from the finalized block at height $H_i(B_{last})$ as specified in the message body and a datastructure of the following format: $C=((r, \pi), H_i(B_{last}),id_\mathbb{V})$

where
- $(r, \pi) \leftarrow ETF.Sign(d_c, H_1(ID_(B_{last})))$ is the IBE secret and DLEQ proof calculated for the given slot identity.
- $H_i(B_{last})$ is the block number this commitment is for. Namely the latest, finalized block.
- $id_\mathbb{V}$ is the current authority set Id.

A **vote message**, $M_v$​, is direct vote created by the Polkadot Host on every ETF round and is gossiped to its peers. The message is a datastructure of the following format: $M_v​=Enc_{SC}​(C, d_i, \sigma_i)$

where

    CC is the BEEFY commitment (Definition 55).
    AidbfyAidbfy​ is the ECDSA public key of the Polkadot Host.
    AsigAsig​ is the signature created with AidbfyAidbfy​ by signing the payload RhRh​ in CC.

**Payload** is the Merkle root of the MMR generated where the leaf data contains the following fields for each block:

- LeafVersion: a byte indicating the current version number of the Leaf Format. The first 3 bits are for major versions and the last 5 bits are for minor versions.
- EtfNextAuthoritySetInfo: It is a tuple consisting of:
    - ValidatorSetID
    - Len (u32): length of the validator set
    - Merkle Root of the list of Next Beefy Authority Set (ECDSA public keys). The exact format depends on the implementation. 
- Parent Block number and Parent Block Hash.
- Extra Leaf Data: Currently the Merkle root of the list of (ParaID, ParaHeads)