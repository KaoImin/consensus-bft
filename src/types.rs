use crate::consensus::INIT_HEIGHT;
use bft_core::types as bft;
use rlp::{Decodable, Encodable, RlpStream};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::collections::HashMap;

///
pub type Address = Vec<u8>;
///
pub type Hash = Vec<u8>;

///
#[derive(Debug, Clone)]
pub enum ConsensusInput<
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
> {
    ///
    SignedProposal(SignedProposal<F>),
    ///
    SignedVote(SignedVote),
    ///
    Status(Status),
}

#[derive(Debug, Clone)]
pub(crate) enum AsyncMsg<
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
> {
    ///
    VerifyResp(VerifyResp),
    ///
    Feed(Feed<F>),
}

///
#[derive(Debug, Clone)]
pub enum ConsensusOutput<
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
> {
    ///
    SignedProposal(SignedProposal<F>),
    ///
    SignedVote(SignedVote),
    ///
    Commit(Commit<F>),
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
            VoteType::Precommit => 1,
        }
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SignedProposal<
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
> {
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub proposal: Proposal<F>,
    ///
    pub signature: Vec<u8>,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proposal<
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
> {
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub content: F,
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub proof: Proof<F>,
    ///
    pub lock_round: Option<u64>,
    ///
    pub lock_votes: Vec<SignedVote>,
    ///
    pub proposer: Address,
}

impl<F> Encodable for Proposal<F>
where
    F: Encodable + Decodable + Encodable + Clone + Send + 'static + Serialize + DeserializeOwned,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.height)
            .append(&self.round)
            .append(&self.content);
        if let Some(lock_round) = self.lock_round {
            s.append(&lock_round).append_list(&self.lock_votes);
        }
        s.append(&self.proposer);
    }
}

impl<F> Proposal<F>
where
    F: Encodable + Decodable + Encodable + Clone + Send + 'static + Serialize + DeserializeOwned,
{
    ///
    pub fn to_bft_proposal(&self, hash: Vec<u8>) -> bft::Proposal {
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
    pub proposal: Hash,
    ///
    pub voter: Address,
}

impl Encodable for Vote {
    fn rlp_append(&self, s: &mut RlpStream) {
        let res: u8 = self.vote_type.clone().into();
        s.append(&res)
            .append(&self.height)
            .append(&self.round)
            .append(&self.proposal)
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
            proposal: self.proposal.clone(),
            voter: self.voter.clone(),
        }
    }

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

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Commit<F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned>
{
    ///
    pub height: u64,
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub result: F,
    ///
    pub prev_hash: Hash,
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub proof: Proof<F>,
    ///
    pub address: Address,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Status {
    ///
    pub height: u64,
    ///
    pub prev_hash: Hash,
    ///
    pub interval: Option<u64>,
    ///
    pub authority_list: Vec<Node>,
}

impl Status {
    ///
    pub fn to_bft_status(&self) -> bft::Status {
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
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Feed<F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned> {
    /// The height of the proposal.
    pub height: u64,
    /// A proposal.
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub content: F,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VerifyResp {
    ///
    pub is_pass: bool,
    ///
    pub proposal: Hash,
}

impl VerifyResp {
    pub(crate) fn to_bft_resp(&self) -> bft::VerifyResp {
        bft::VerifyResp {
            is_pass: self.is_pass,
            proposal: self.proposal.clone(),
        }
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
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

impl Node {
    ///
    pub fn new(address: Address) -> Self {
        Node {
            address,
            proposal_weight: 1,
            vote_weight: 1,
        }
    }

    ///
    pub fn get_address(&self) -> Address {
        self.address.clone()
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Proof<F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned> {
    ///
    #[serde(bound(deserialize = "F: DeserializeOwned"))]
    pub block_hash: F,
    ///
    pub height: u64,
    ///
    pub round: u64,
    ///
    pub precommit_votes: HashMap<Address, Vec<u8>>,
}

impl<F> Encodable for Proof<F>
where
    F: Encodable + Decodable + Encodable + Clone + Send + 'static + Serialize + DeserializeOwned,
{
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append(&self.block_hash)
            .append(&self.height)
            .append(&self.round);

        let votes = &self
            .precommit_votes
            .iter()
            .map(|(k, v)| (k.to_vec(), v.to_vec()))
            .collect::<Vec<_>>();
        for v in votes.iter() {
            s.append(&v.0).append(&v.1);
        }
    }
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LockStatus {
    ///
    pub block: Hash,
    ///
    pub round: u64,
    ///
    pub votes: Vec<SignedVote>,
}
