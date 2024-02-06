use sc_cli::{
    NodeKeyParams,
    SharedParams,
	CliConfiguration,
};
use clap::Parser;
use log::info;
use sc_network::config::build_multiaddr;
use sc_service::{
	config::{MultiaddrWithPeerId, NetworkConfiguration},
	ChainSpec,
};
use serde::{Serialize, Deserialize};
use serde_json::{Value};
use std::fs::read_to_string;
use std::{
	fs,
	io::{Read, Write},
	path::PathBuf,
};
use paillier::{DecryptionKey, EncryptionKey, Keypair};
use crate::errors::Error;

use etf_crypto_primitives::dpss::acss::{
	ACSSParams, 
	HighThresholdACSS, 
	WrappedEncryptionKey
};
use ark_ff::UniformRand;
use ark_bls12_381::Fr;
use rand_chacha::{
	ChaCha20Rng,
	rand_core::SeedableRng,
};

/// The `init` command used to build shares for an initial committee (ACSS.Reshare)
#[derive(Debug, Clone, Parser)]
pub struct InitEtfCmd {
	
	/// A file to write the output shares to
	#[arg(long)]
	output: Option<PathBuf>,

	/// A file containing the public keys of the committee
	#[arg(long)]
	committee: PathBuf,

	#[arg(long)]
	pub seed: u64,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub shared_params: SharedParams,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub node_key_params: NodeKeyParams,
}


impl InitEtfCmd {
	/// run the command
	pub fn run(
		&self,
	) -> Result<(), Error> {
		info!("Generating secrets for initial committee: Running ACSS::reshare");

		let filepath = self.committee.clone().into_os_string().into_string().unwrap();
		let committee: Vec<WrappedEncryptionKey> = 
			read_to_string(filepath) 
			.unwrap()
			.lines()
			.map(|data| {
				let bytes = array_bytes::hex2bytes(&data).unwrap();
				let ek: EncryptionKey = bincode::deserialize(&bytes).unwrap();
				WrappedEncryptionKey(ek)
			})
			.collect::<Vec<_>>();

		let mut rng = ChaCha20Rng::seed_from_u64(self.seed);

		// generate msks
		let msk = Fr::rand(&mut rng);
		let msk_prime = Fr::rand(&mut rng);

		let params = ACSSParams::rand(&mut rng);

		let shares_map = HighThresholdACSS::produce_shares(
			params, 
			msk, 
			msk_prime,
			&committee,
			(committee.len() - 1) as u8,
			&mut rng,
		);

		let bytes = bincode::serialize(&shares_map).unwrap();
		let file_data = array_bytes::bytes2hex("", bytes).into_bytes();

		match &self.output {
			Some(file) => fs::write(file, file_data).map_err(|_| Error::BadFile)?,
			_ => {
				std::io::stdout()
					.write_all(b"Failed to locate file, printing shares: ")
					.map_err(|_| Error::WriteError)?;
				std::io::stdout()
					.write_all(&file_data)
					.map_err(|_| Error::WriteError)?;
			},
		}

		Ok(())
	}
}

impl CliConfiguration for InitEtfCmd {
	fn shared_params(&self) -> &SharedParams {
		&self.shared_params
	}

	fn node_key_params(&self) -> Option<&NodeKeyParams> {
		Some(&self.node_key_params)
	}
}
