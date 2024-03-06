/*
 * Copyright 2024 by Ideal Labs, LLC
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
use sc_cli::{
    NodeKeyParams,
    SharedParams,
	CliConfiguration,
};
use clap::Parser;
use log::info;
use etf_crypto_primitives::{
	MinimalDecryptionKey,
	utils::{
		paillier_create_keypair, 
		paillier_create_keys, 
		KeypairWrapper,
	}
};
use std::{
	fs,
	io,
	io::{Write, Read},
	path::PathBuf,
};

use crate::errors::Error;

/// The `build-spec` command used to build a specification.
#[derive(Debug, Clone, Parser)]
pub struct PaillierInspectCmd {

	/// Name of file to read keypair bytes from
	#[arg(long)]
	keys: PathBuf,

	/// Name of the file to output the public key bytes to
	/// if not given, the public key is printed
	#[arg(long)]
	output: Option<PathBuf>,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub shared_params: SharedParams,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub node_key_params: NodeKeyParams,
}

impl PaillierInspectCmd {
    /// run the command
	pub fn run(
		&self,
	) -> Result<(), Error> {
		let mut file_data = fs::read(&self.keys).map_err(|_| Error::BadFile)?;
		if let Ok(kp_bytes) = array_bytes::hex2bytes(file_data) {
			let kp: etf_crypto_primitives::utils::KeypairWrapper = 
				serde_json::from_slice(&kp_bytes)
					.map_err(|_| Error::WriteError)?;
			// recover only the public key
			let bytes = serde_json::to_vec(&kp.ek).unwrap();
			let file_data = array_bytes::bytes2hex("", bytes).into_bytes();

			match &self.output {
				Some(file) => {
					let dk = MinimalDecryptionKey::from(&kp.dk);
					let bytes = serde_json::to_vec(&dk).unwrap();
					let hex = array_bytes::bytes2hex("", bytes);

					std::io::stdout().write_all(&hex.as_bytes()).map_err(|_| Error::WriteError)?;
					fs::write(file, file_data).map_err(|_| Error::BadFile)?;
				},
				_ => {
					std::io::stdout()
						.write_all(b"Failed to locate file, only printing public key")
						.map_err(|_| Error::WriteError)?;
				},
			}	
		}
		Ok(())
	}
}


impl CliConfiguration for PaillierInspectCmd {
	fn shared_params(&self) -> &SharedParams {
		&self.shared_params
	}

	fn node_key_params(&self) -> Option<&NodeKeyParams> {
		Some(&self.node_key_params)
	}
}

