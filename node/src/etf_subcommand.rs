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

//! Paillier Key related CLI utilities

use crate::{
    errors::Error, 
    paillier_keygen::PaillierKeygenCmd, 
    paillier_inspect::PaillierInspectCmd,
	init_etf::InitEtfCmd,
};
use sc_cli::SubstrateCli;


/// Key utilities for the cli.
#[derive(Debug, clap::Subcommand)]
pub enum EtfSubcommand {
	/// Generate a random node key, write it to a file or stdout and write the
	/// corresponding peer-id to stderr
	Generate(PaillierKeygenCmd),

    /// outputs the public key to stdout
    Inspect(PaillierInspectCmd),

	Init(InitEtfCmd),
}

impl EtfSubcommand {
	/// run the key subcommands
	pub fn run<C: SubstrateCli>(&self, cli: &C) -> Result<(), Error> {
		match self {
			EtfSubcommand::Generate(cmd) => cmd.run(),
            EtfSubcommand::Inspect(cmd) => cmd.run(),
			EtfSubcommand::Init(cmd) => cmd.run(),
		}
	}
}
