//!
//!

#![deny(missing_docs)]
#![warn(unused_imports)]
#![warn(dead_code)]

use crate::types::{Address, Commit, ConsensusOutput};
use rlp::{Decodable, Encodable};

///
pub trait ConsensusSupport<F: Encodable + Decodable + Clone + Send + 'static> {
    type Error: ::std::fmt::Debug;
    ///
    fn get_block(&self, height: u64) -> Result<Vec<u8>, Self::Error>;
    ///
    fn transmit(&self, msg: ConsensusOutput<F>) -> Result<(), Self::Error>;
    ///
    fn check_block(&self, block: &[u8]) -> Result<Vec<u8>, Self::Error>;
    ///
    fn commit(&self, commit: Commit<F>) -> Result<(), Self::Error>;
    ///
    fn sign(&self, hash: &[u8]) -> Result<Vec<u8>, Self::Error>;
    ///
    fn check_signature(&self, signature: &[u8], hash: &[u8]) -> Result<Address, Self::Error>;
    ///
    fn hash(&self, msg: &[u8]) -> Vec<u8>;
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
pub mod util;
///
pub mod wal;
