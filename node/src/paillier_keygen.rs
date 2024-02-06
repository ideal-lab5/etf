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
use serde::{Serialize, Deserialize};
use etf_crypto_primitives::utils::{
	paillier_create_keypair, 
	paillier_create_keys, 
	KeypairWrapper};
use std::{
	fs,
	io::Write,
	path::PathBuf,
};

use crate::errors::Error;

/// The `build-spec` command used to build a specification.
#[derive(Debug, Clone, Parser)]
pub struct PaillierKeygenCmd {

	/// Name of file to save secret key to.
	/// If not given, the secret key is printed to stdout.
	#[arg(long)]
	file: Option<PathBuf>,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub shared_params: SharedParams,

	#[allow(missing_docs)]
	#[clap(flatten)]
	pub node_key_params: NodeKeyParams,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
	pub ek: Vec<u8>,
	pub dk: Vec<u8>,
}

impl PaillierKeygenCmd {
    /// run the command
	pub fn run(
		&self,
	) -> Result<(), Error> {
		info!("Generating Paillier keypair");

        if let Ok(keypair_bytes) = paillier_create_keypair() {
			if let Ok(keys_bytes) = paillier_create_keys(keypair_bytes.clone()) {
				let file_data = array_bytes::bytes2hex("", keys_bytes.clone().as_slice()).into_bytes();

				match &self.file {
					Some(file) => fs::write(file, file_data).map_err(|_| Error::BadFile)?,
					_ => {
						std::io::stdout()
							.write_all(b"Failed to locate file, printing keypair: ")
							.map_err(|_| Error::WriteError)?;
						std::io::stdout()
						.write_all(&file_data)
							.map_err(|_| Error::WriteError)?;
					},
				}		
			}
		}
		Ok(())
	}
}


impl CliConfiguration for PaillierKeygenCmd {
	fn shared_params(&self) -> &SharedParams {
		&self.shared_params
	}

	fn node_key_params(&self) -> Option<&NodeKeyParams> {
		Some(&self.node_key_params)
	}
}
