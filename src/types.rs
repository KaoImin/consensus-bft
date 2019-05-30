use crate::{consensus::INIT_HEIGHT, Content};

use bft_core::types as bft;
use log::error;
use rlp::{Decodable, DecoderError, Encodable, Prototype, Rlp, RlpStream};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use std::collections::HashMap;

/// Address type.
pub type Address = Vec<u8>;
/// Hash type.
pub type Hash = Vec<u8>;
/// Signature type.
pub type Signature = Vec<u8>;

/// Consensus input message types.
#[derive(Debug, Clone)]
pub enum ConsensusInput<F: Content + Sync> {
    /// Signed proposal message.
    SignedProposal(SignedProposal<F>),
    /// Signed vote message.
    SignedVote(SignedVote),
    /// Rich status message.
    Status(Status),
}

/// Async message type.
#[derive(Debug, Clone)]
pub(crate) enum AsyncMsg<F: Content + Sync> {
    /// Verify response of a proposal.
    VerifyResp(VerifyResp),
    /// Transcation set of a height.
    Feed(Feed<F>),
}

/// Consensus output message.
#[derive(Debug, Clone)]
pub enum ConsensusOutput<F: Content + Sync> {
    /// Signed proposal message.
    SignedProposal(SignedProposal<F>),
    /// Signed vote message.
    SignedVote(SignedVote),
    /// Commit message.
    Commit(Commit<F>),
}

/// Vote types.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub enum VoteType {
    /// Prevote vote type.
    Prevote,
    /// Precommit vote type.
    Precommit,
}

impl From<u8> for VoteType {
    fn from(s: u8) -> Self {
        match s {
            0 => VoteType::Prevote,
            1 => VoteType::Precommit,
            _ => panic!("Invalid vote type!"),
        }
    }
}

impl Into<u8> for VoteType {
    fn into(self) -> u8 {
        match self {
            VoteType::Prevote => 0,
            VoteType::Precommit => 1,
        }
    }
}

/// A signed proposal.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignedProposal<F: Content + Sync> {
    /// A proposal.
    pub proposal: Proposal,
    /// A signature of the proposal.
    pub signature: Signature,
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    /// The block content.
    pub content: F,
}

impl<F> Encodable for SignedProposal<F>
where
    F: Content + Sync,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(3)
            .append(&self.proposal)
            .append(&self.signature)
            .append(&self.content);
    }
}

impl<F> Decodable for SignedProposal<F>
where
    F: Content + Sync,
{
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(3) => {
                let proposal: Proposal = r.val_at(0)?;
                let signature: Signature = r.val_at(1)?;
                let content: F = r.val_at(2)?;
                Ok(SignedProposal {
                    proposal,
                    signature,
                    content,
                })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

impl<F: Content + Sync> SignedProposal<F> {
    ///
    pub fn get_block(&self) -> F {
        self.content.clone()
    }
}

/// A Proposal.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal {
    /// The height of a proposal.
    pub height: u64,
    /// The round of a proposal.
    pub round: u64,
    /// The content of a proposal.
    pub hash: Hash,
    /// The proof of a proposal.
    pub proof: Proof,
    /// The lock round of a proposal. If the proposal has not been locked, it should be `None`.
    pub lock_round: Option<u64>,
    /// The lock votes of a proposal. If the proposal has not been locked, it should be an empty `Vec`.
    pub lock_votes: Vec<SignedVote>,
    /// The address of proposer.
    pub proposer: Address,
}

impl Encodable for Proposal {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(7)
            .append(&self.height)
            .append(&self.round)
            .append(&self.hash)
            .append(&self.proof)
            .append(&self.lock_round)
            .append_list(&self.lock_votes)
            .append(&self.proposer);
    }
}

impl Decodable for Proposal {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(7) => {
                let height: u64 = r.val_at(0)?;
                let round: u64 = r.val_at(1)?;
                let hash: Hash = r.val_at(2)?;
                let proof: Proof = r.val_at(3)?;
                let lock_round: Option<u64> = r.val_at(4)?;
                let lock_votes: Vec<SignedVote> = r.list_at(5)?;
                let proposer: Address = r.val_at(6)?;
                Ok(Proposal {
                    height,
                    round,
                    hash,
                    proof,
                    lock_round,
                    lock_votes,
                    proposer,
                })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

impl Proposal {
    /// A function to convert a proposal into the corresponding type in BFT-core.
    pub(crate) fn to_bft_proposal(&self, hash: Vec<u8>) -> bft::Proposal {
        let lock_votes = if self.lock_round.is_some() {
            let mut res = Vec::new();
            for vote in self.lock_votes.iter() {
                res.push(vote.vote.to_bft_vote());
            }
            res
        } else {
            Vec::new()
        };

        bft::Proposal {
            height: self.height,
            round: self.round,
            content: hash,
            lock_round: self.lock_round,
            lock_votes,
            proposer: self.proposer.clone(),
        }
    }
}

/// A signed vote.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignedVote {
    /// A vote.
    pub vote: Vote,
    /// A signature of the vote.
    pub signature: Signature,
}

impl Encodable for SignedVote {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(2)
            .append(&self.vote)
            .append_list(&self.signature);
    }
}

impl Decodable for SignedVote {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(2) => {
                let vote: Vote = r.val_at(0)?;
                let signature: Signature = r.val_at(1)?;
                Ok(SignedVote { vote, signature })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

/// A vote.
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Vote {
    /// The vote type of a vote.
    pub vote_type: VoteType,
    /// The height of a vote.
    pub height: u64,
    /// The round of a vote.
    pub round: u64,
    /// The proposal of a vote.
    pub proposal: Hash,
    /// The address of voter.
    pub voter: Address,
}

impl Encodable for Vote {
    fn rlp_append(&self, s: &mut RlpStream) {
        let res: u8 = self.vote_type.clone().into();
        s.begin_list(5)
            .append(&res)
            .append(&self.height)
            .append(&self.round)
            .append(&self.proposal)
            .append(&self.voter);
    }
}

impl Decodable for Vote {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(5) => {
                let vote_type: u8 = r.val_at(0)?;
                let vote_type: VoteType = VoteType::from(vote_type);
                let height: u64 = r.val_at(1)?;
                let round: u64 = r.val_at(2)?;
                let proposal: Hash = r.val_at(3)?;
                let voter: Address = r.val_at(4)?;
                Ok(Vote {
                    vote_type,
                    height,
                    round,
                    proposal,
                    voter,
                })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

impl Vote {
    /// A function to convert vote into the corresponding type in BFT-core.
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
            proposal: self.proposal.clone(),
            voter: self.voter.clone(),
        }
    }

    /// A function to convert vote from the corresponding type in BFT-core.
    pub(crate) fn from_bft_vote(vote: bft::Vote, vtype: VoteType) -> Vote {
        Vote {
            vote_type: vtype,
            height: vote.height,
            round: vote.round,
            proposal: vote.proposal,
            voter: vote.voter,
        }
    }
}

/// A commit.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Commit<F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned>
{
    /// The height of a commit.
    pub height: u64,
    /// The consensus result.
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub result: F,
    /// The previous hash.
    pub prev_hash: Hash,
    /// The proof of the commit.
    pub proof: Proof,
    /// The address of the node.
    pub address: Address,
}

impl<F> Encodable for Commit<F>
where
    F: Content + Sync,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&self.height)
            .append(&self.result)
            .append(&self.proof)
            .append(&self.prev_hash)
            .append(&self.address);
    }
}

impl<F> Decodable for Commit<F>
where
    F: Content + Sync,
{
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(5) => {
                let height: u64 = r.val_at(0)?;
                let result: F = r.val_at(1)?;
                let proof: Proof = r.val_at(2)?;
                let prev_hash: Hash = r.val_at(3)?;
                let address: Address = r.val_at(4)?;
                Ok(Commit {
                    height,
                    result,
                    proof,
                    prev_hash,
                    address,
                })
            }
            _ => Err(DecoderError::RlpInconsistentLengthAndData),
        }
    }
}

/// A rich status.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// The height of a status.
    pub height: u64,
    /// The block hash of the height.
    pub prev_hash: Hash,
    /// The consensus interval. If it is `None`, use the default interval that is 3 seconds.
    pub interval: Option<u64>,
    /// The authority of the next height.
    pub authority_list: Vec<Node>,
}

impl Status {
    /// A function to convert a rich status into the corresponding type in BFT-core.
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

/// A feed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Feed<F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned> {
    /// The height of the proposal.
    pub height: u64,
    /// A proposal.
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub content: F,
    /// The proposal hash.
    pub hash: Hash,
}

/// Verify response type.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct VerifyResp {
    /// A result of verify.
    pub(crate) is_pass: bool,
    /// A proposal.
    pub(crate) proposal: Hash,
}

impl VerifyResp {
    /// A function to convert verify response into the corresponding type in BFT-core.
    pub(crate) fn to_bft_resp(&self) -> bft::VerifyResp {
        bft::VerifyResp {
            is_pass: self.is_pass,
            proposal: self.proposal.clone(),
        }
    }
}

/// An authority manage.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AuthorityManage {
    /// The authority list at present.
    pub authorities: Vec<Node>,
    /// An old authority.
    pub authorities_old: Vec<Node>,
    /// The height of the old authority.
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
    /// A function to update authority list.
    pub(crate) fn update_authority(&mut self, h: u64, auth_list: Vec<Node>) {
        let tmp = self.authorities.clone();
        self.authorities = auth_list;
        self.authorities_old = tmp;
        self.authority_h_old = h;
    }
}

/// A node.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// The address of a node.
    pub address: Address,
    /// The proposal weight of a node.
    pub proposal_weight: u32,
    /// The vote weight of a node.
    pub vote_weight: u32,
}

impl Node {
    /// A function to generate a `Node` with default proposal weight and vote weight.
    pub fn new(address: Address) -> Self {
        Node {
            address,
            proposal_weight: 1,
            vote_weight: 1,
        }
    }

    /// A function to get the address of a node.
    pub fn get_address(&self) -> Address {
        self.address.clone()
    }
}

/// A proof.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proof {
    /// The hash of a proof.
    pub block_hash: Hash,
    /// The height of votes in the proof.
    pub height: u64,
    /// The rounf of votes in the proof.
    pub round: u64,
    /// The precommit vote set of the proof.
    pub precommit_votes: HashMap<Address, Vec<u8>>,
}

impl Encodable for Proof {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.begin_list(5)
            .append(&self.height)
            .append(&self.round)
            .append(&self.block_hash);
        let mut key_values: Vec<(Address, Signature)> =
            self.precommit_votes.clone().into_iter().collect();
        key_values.sort();
        let mut key_list: Vec<Address> = vec![];
        let mut value_list: Vec<Signature> = vec![];
        key_values.iter().for_each(|(address, sig)| {
            key_list.push(address.to_owned());
            value_list.push(sig.to_owned());
        });
        s.begin_list(key_list.len());
        for key in key_list {
            s.append(&key);
        }
        s.begin_list(value_list.len());
        for value in value_list {
            s.append(&value);
        }
    }
}

impl Decodable for Proof {
    fn decode(r: &Rlp) -> Result<Self, DecoderError> {
        match r.prototype()? {
            Prototype::List(5) => {
                let height: u64 = r.val_at(0)?;
                let round: u64 = r.val_at(1)?;
                let block_hash: Hash = r.val_at(2)?;
                let key_list: Vec<Address> = r.list_at(3)?;
                let value_list: Vec<Signature> = r.list_at(4)?;
                if key_list.len() != value_list.len() {
                    error!(
                        "Decode proof error, key_list_len {}, value_list_len{}",
                        key_list.len(),
                        value_list.len()
                    );
                    return Err(DecoderError::RlpIncorrectListLen);
                }
                let precommit_votes: HashMap<_, _> =
                    key_list.into_iter().zip(value_list.into_iter()).collect();
                Ok(Proof {
                    height,
                    round,
                    block_hash,
                    precommit_votes,
                })
            }
            _ => {
                error!("Decode proof error, the prototype is {:?}", r.prototype());
                Err(DecoderError::RlpInconsistentLengthAndData)
            }
        }
    }
}
