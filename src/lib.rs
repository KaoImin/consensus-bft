//!
//!

#![deny(missing_docs)]
#![warn(unused_imports)]
#![warn(dead_code)]

use crate::types::{Address, Commit, ConsensusOutput};
use rlp::{Decodable, Encodable};
use serde::{de::DeserializeOwned, ser::Serialize};
use std::fmt::Debug;

/// Consensus support
pub trait ConsensusSupport<F: Content + Sync> {
    /// Support error type.
    type Error: Debug;
    /// Get a proposal content of a height. If success, return `Ok(F)` that `F`
    /// is an example of `Content`, else return `Err()`.
    fn get_content(&self, height: u64) -> Result<F, Self::Error>;
    /// Transmit a consensus output to other nodes.
    fn transmit(&self, msg: ConsensusOutput<F>) -> Result<(), Self::Error>;
    /// Check the validity of the transcations of a proposal. If success return `Ok(())`,
    /// else return `Err()`.
    fn check_proposal(&self, block: F, height: u64) -> Result<(), Self::Error>;
    /// Do commit.
    fn commit(&self, commit: Commit<F>) -> Result<(), Self::Error>;
    /// Use the given hash and private key to sign a signature. If success, return `Ok(signature)`,
    /// else return `Err()`.
    fn sign(&self, hash: &[u8]) -> Result<Vec<u8>, Self::Error>;
    /// Verify a signature. If success return a `Ok(address)` that recover from the given signature
    /// and hash, else return `Err()`.
    fn verify_signature(&self, signature: &[u8], hash: &[u8]) -> Result<Address, Self::Error>;
    /// Hash a message.
    fn hash(&self, msg: &[u8]) -> Vec<u8>;
}

/// A trait define the proposal content, wrapper `Decodable`, `Encodable`, `Clone`, `Debug`,
/// `Send`, `'static`, `Serialize` and `Deserialize`.
pub trait Content:
    Encodable + Decodable + Clone + Debug + Send + 'static + Serialize + DeserializeOwned
{
}

/// Vote collection and proposal collection.
pub(crate) mod collection;
/// Consensus survice.
pub mod consensus;
/// Consensus error.
pub mod error;
/// Types used in consensus.
pub mod types;
/// Some utils.
pub mod util;
/// Consensus wal log.
pub(crate) mod wal;

/// Re-pub consensus executor
pub use consensus::ConsensusExecutor;
/// Re-pub check proof function.
pub use util::check_proof;
