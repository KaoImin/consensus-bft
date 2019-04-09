//!
//!

#![deny(missing_docs)]
#[warn(unused_imports)]
extern crate bft_rs as bft;
extern crate bincode;
extern crate blake2b_simd as blake2b;
extern crate cita_crypto as crypto;
#[macro_use]
extern crate crossbeam;
extern crate ethereum_types;
#[macro_use]
extern crate log;
extern crate lru_cache;
extern crate rlp;
#[macro_use]
extern crate serde_derive;

///
pub mod collection;
///
pub mod consensus;
///
pub mod error;
///
pub mod wal;

use crate::{
    consensus::{INIT_HEIGHT, PLACEHOLDER},
    error::ConsensusError,
};
use bincode::serialize;
use crypto::Signature;
use rlp::{Encodable, RlpStream};
use std::collections::HashMap;

///
pub type Address = Vec<u8>;
///
pub type Hash = Vec<u8>;

///
pub enum ConsensusInput {
    ///
    SignedProposal(SignedProposal),
    ///
    SignedVote(SignedVote),
    ///
    Status(Status),
    ///
    VerifyResp(VerifyResp),
    ///
    Feed(Feed),
    ///
    Pause,
    ///
    Start,
}

///
pub enum ConsensusOutput {
    ///
    SignedProposal(SignedProposal),
    ///
    SignedVote(SignedVote),
}

///
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum VoteType {
    ///
    Prevote,
    ///
    Precommit,
}

impl Into<u8> for VoteType {
    fn into(self) -> u8 {
        match self {
            VoteType::Prevote => 0,
            VoteType::Prevote => 1,
            _ => panic!("Invalid type"),
        }
    }
}

///
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Message {
    mtype: String,
    msg: Vec<u8>,
}

impl Message {
    pub(crate) fn from_proposal(signed_proposal: SignedProposal) -> Message {
        Message {
            mtype: "SignedProposal".to_string(),
            msg: serialize(&signed_proposal).expect("Serialize SignedProposal Fail!"),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
///
pub struct SignedProposal {
    ///
    pub proposal: Proposal,
    ///
    pub signature: Vec<u8>,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
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
}

impl Encodable for Proposal {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.height)
            .append(&self.round)
            .append(&self.block);
        if let Some(lock_round) = self.lock_round {
            s.append(&lock_round).append_list(&self.lock_votes);
        }
        s.append(&self.proposer);
    }
}

impl Proposal {
    pub(crate) fn to_bft_proposal(&self) -> bft::Proposal {
        let lock_votes = if self.lock_round.is_some() {
            let mut res = Vec::new();
            for vote in self.lock_votes.iter() {
                res.push(vote.vote.to_bft_vote());
            }
            Some(res)
        } else {
            None
        };

        bft::Proposal {
            height: self.height,
            round: self.round,
            content: self.block.clone(),
            lock_round: self.lock_round,
            lock_votes,
            proposer: self.proposer.clone(),
        }
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignedVote {
    ///
    pub vote: Vote,
    ///
    pub signature: Vec<u8>,
}

impl Encodable for SignedVote {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.vote).append_list(&self.signature);
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Vote {
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
}

impl Encodable for Vote {
    fn rlp_append(&self, s: &mut RlpStream) {
        match self.vote_type {
            VoteType::Prevote => s.append(&(0 as u8)),
            VoteType::Precommit => s.append(&(1 as u8)),
            _ => panic!(""),
        };
        s.append(&self.height)
            .append(&self.round)
            .append(&self.block_hash)
            .append(&self.voter);
    }
}

impl Vote {
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
            proposal: self.block_hash.clone(),
            voter: self.voter.clone(),
        }
    }

    pub(crate) fn from_bft_vote(vote: bft::Vote, vtype: VoteType) -> Vote {
        Vote {
            vote_type: vtype,
            height: vote.height,
            round: vote.round,
            block_hash: vote.proposal,
            voter: vote.voter,
        }
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    ///
    pub height: u64,
    ///
    pub block: Vec<u8>,
    ///
    pub pre_hash: Hash,
    ///
    pub proof: Proof,
    ///
    pub address: Address,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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
            res.push(node.address.to_owned());
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

impl Feed {
    pub(crate) fn to_bft_feed(&self, proposal: Vec<u8>) -> bft::Feed {
        bft::Feed {
            height: self.height,
            proposal,
        }
    }
}

///
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifyResp {
    ///
    pub is_pass: bool,
    ///
    pub block_hash: Hash,
}

impl VerifyResp {
    fn to_BftResp(&self) -> bft::VerifyResp {
        bft::VerifyResp {
            is_pass: self.is_pass,
            proposal: self.block_hash.clone(),
        }
    }
}

///
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

impl AuthorityManage {
    pub(crate) fn update_authority(&mut self, h: u64, auth_list: Vec<Node>) {
        let tmp = self.authorities.clone();
        self.authorities = auth_list;
        self.authorities_old = tmp;
        self.authority_h_old = h;
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Node {
    ///
    pub address: Address,
    ///
    pub proposal_weight: u32,
    ///
    pub vote_weight: u32,
}

// impl Node {
//     pub(crate) fn to_bft_node(&self) -> bft::Node {
//         bft::Node {
//             address: self.address,
//             propose_weight: self.proposal_weight,
//             vote_weight: self.vote_weight,
//         }
//     }
// }

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    ///
    pub block_hash: Hash,
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    pub precommit_votes: HashMap<Address, Signature>,
}

impl Encodable for Proof {
    fn rlp_append(&self, s: &mut RlpStream) {
        let tmp = self.precommit_votes.clone();
        s.append_list(&self.block_hash)
            .append(&self.height)
            .append(&self.round);
        for (k, v) in tmp.iter() {
            s.append_list(&k).append_list(&v.0.to_vec());
        }
    }
}

///
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
    fn transmit(&self, msg: ConsensusOutput) -> Result<bool, ConsensusError>;
    ///
    fn check_block(&self, block: &[u8]) -> Result<bool, ConsensusError>;
    ///
    fn commit(&self, commit: Commit) -> Result<(), ConsensusError>;
    ///
    fn signature(&self, hash: &[u8]) -> Option<Vec<u8>>;
    ///
    fn check_signature(&self, signature: &[u8], hash: &[u8]) -> Option<Address>;
    ///
    fn crypt_hash(&self, msg: &[u8]) -> Vec<u8>; 
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
