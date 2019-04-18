//!
//!

#![deny(missing_docs)]
#![warn(unused_imports)]
#![warn(dead_code)]
extern crate bft_rs as bft;
extern crate crossbeam;
#[macro_use]
extern crate log;
extern crate lru_cache;
extern crate rlp;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

use crate::error::ConsensusError;
use crate::types::{Address, ConsensusOutput, Commit, Feed};

///
pub trait ConsensusSupport {
    ///
    fn get_block(&self, height: u64) -> Result<Feed, ConsensusError>;
    ///
    fn transmit(&self, msg: ConsensusOutput) -> Result<(), ConsensusError>;
    ///
    fn check_block(&self, block: &[u8]) -> bool;
    ///
    fn commit(&self, commit: Commit) -> Result<(), ConsensusError>;
    ///
    fn signature(&self, hash: &[u8]) -> Option<Vec<u8>>;
    ///
    fn check_signature(&self, signature: &[u8], hash: &[u8]) -> Option<Address>;
    ///
    fn crypt_hash(&self, msg: &[u8]) -> Vec<u8>;
}

///
pub mod collection;
///
pub mod consensus;
///
pub mod error;
///
pub mod types;
///
pub mod wal;