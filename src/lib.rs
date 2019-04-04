//!
//!

#![deny(missing_docs)]

extern crate bft_rs;
extern crate cita_crypto;
#[macro_use]
extern crate log;
extern crate rlp;

pub mod consensus;
pub mod error;
pub mod wal;

use crate::{consensus::INIT_HEIGHT, error::ConsensusError};
use bft_rs as bft;
use std::collections::HashMap;

///
pub type Address = Vec<u8>;
///
pub type Hash = Vec<u8>;

///
pub enum ConsensusInput {
    SignedProposal(SignedProposal),
    SignedVote(SignedVote),
    Status(Status),
    VerifyResp(VerifyResp),
    Feed(Feed),
    Pause,
    Start,
}

///
pub enum ConsensusOutput {
    SignedProposal(SignedProposal),
    SignedVote(SignedVote),
}

///
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteType {
    Prevote,
    Precommit,
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedProposal {
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    pub block: Vec<u8>,
    ///
    pub proof: Proof,
    ///
    pub lock_round: Option<u64>,
    ///
    pub lock_votes: Vec<SignedVote>,
    ///
    pub proposer: Address,
    ///
    pub signature: Vec<u8>,
}

impl SignedProposal {
    pub(crate) fn to_bft_proposal(&self) -> bft::Proposal {
        let lock_votes = if self.lock_round.is_some() {
            let mut res = Vec::new();
            for vote in self.lock_votes.iter() {
                res.push(vote.to_bft_vote());
            }
            res
        } else {
            Vec::new()
        };

        bft::Proposal {
            height: self.height,
            round: self.round,
            content: self.block,
            lock_round: self.lock_round,
            lock_votes,
            proposer: self.proposer,
        }
    }
}

///
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignedVote {
    ///
    pub vote_type: VoteType,
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    pub block_hash: Hash,
    ///
    pub voter: Address,
    ///
    pub signature: Vec<u8>,
}

impl SignedVote {
    pub(crate) fn to_bft_vote(&self) -> bft::Vote {
        let vote_type = if self.vote_type == VoteType::Prevote {
            bft::VoteType::Prevote
        } else {
            bft::VoteType::Precommit
        };

        bft::Vote {
            vote_type,
            height: self.height,
            round: self.round,
            proposal: self.block_hash,
            voter: self.voter,
        }
    }
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    ///
    pub height: u64,
    ///
    pub block: Vec<u8>,
    ///
    pub pre_hash: Hash,
    ///
    pub proof: Proof ,
    ///
    pub address: Address,
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    ///
    pub height: u64,
    ///
    pub pre_hash: Hash,
    ///
    pub interval: Option<u64>,
    ///
    pub authority_list: Vec<Node>,
}

impl Status {
    pub(crate) fn to_bft_status(&self) -> bft::Status {
        let mut res = Vec::new();
        for node in self.authority_list.iter() {
            res.push(node.to_bft_node());
        }
        bft::Status {
            height: self.height,
            interval: self.interval,
            authority_list: res,
        }
    }
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    /// The height of the proposal.
    pub height: u64,
    /// A proposal.
    pub block: Vec<u8>,
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyResp {
    ///
    pub is_pass: bool,
    ///
    pub block_hash: Hash,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorityManage {
    ///
    pub authorities: Vec<Node>,
    ///
    pub authorities_old: Vec<Node>,
    ///
    pub authority_h_old: u64,
}

impl Default for AuthorityManage {
    fn default() -> Self {
        AuthorityManage {
            authorities: Vec::new(),
            authorities_old: Vec::new(),
            authority_h_old: INIT_HEIGHT,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    ///
    pub address: Address,
    ///
    pub proposal_weight: u32,
    ///
    pub vote_weight: u32,
}

impl Node {
    pub(crate) fn to_bft_node(&self) -> bft::Node {
        bft::Node {
            address: self.address,
            propose_weight: self.proposal_weight,
            vote_weight: self.vote_weight,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    ///
    pub block_hash: Hash,
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    pub precommit_votes: HashMap<Address, Vec<u8>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockStatus {
    ///
    pub block: Hash,
    ///
    pub round: u64,
    ///
    pub votes: Vec<SignedVote>,
}

///
pub trait ConsensusSupport {
    ///
    fn transmit(&self, msg: ConsensusOutput) -> Result<(), ConsensusError>;
    ///
    fn check_block(&self, block: &[u8]) -> Result<(), ConsensusError>;
    ///
    fn commit(&self, commit: Commit) -> Result<(), ConsensusError>;
}

// ///
// pub trait Crypto {
//     /// Hash type
//     type Hash;
//     /// Signature types
//     type Signature: Crypto;
//     /// A function to get signature.
//     fn get_signature(&self) -> Self::Signature;
//     /// A function to encrypt hash.
//     fn hash(&self, msg: Vec<u8>) -> Self::Hash;
//     /// A function to check signature
//     fn check_signature(
//         &self,
//         hash: &Self::Hash,
//         sig: &Self::Signature,
//     ) -> Result<(), ConsensusError>;
//     /// A function to signature the message hash.
//     fn signature(&self: &Self::Hash, privkey: &[u8]) -> Self::Signature;
// }
