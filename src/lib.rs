//!
//!

#![deny(missing_docs)]

extern crate bft_rs;
extern crate cita_crypto;
extern crate rlp;

pub mod consensus;
pub mod error;
pub mod wal;

use bft_rs as bft;

///
pub type Address = Vec<u8>;
///
pub type Hash = Vec<u8>;

pub enum ConsensusInput {
    Proposal(Proposal),
    Vote(Vote),
    Status(Status),
    VerifyResp(VerifyResp),
    Feed(Feed),
    Pause,
    Start,
}

pub enum ConsensusOutput {
    Proposal(Proposal),
    Vote(Vote),
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
    pub lock_votes: Vec<Vote>,
    ///
    pub proposer: Address,
    ///
    pub signature: Signature,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
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
    ///
    pub signature: Signature,
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

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Feed {
    /// The height of the proposal.
    pub height: u64,
    /// A proposal.
    pub block: Vec<u8>,
}

///
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VerifyResp {
    ///
    pub is_pass: bool,
    ///
    pub block_hash: Hash,
}

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
            authorities: Vec::new();
            authorities_old: Vec::new();
            authority_h_old: INIT_HEIHGT,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Node {
    ///
    pub address: Address,
    ///
    pub proposal_weight: u32,
    ///
    pub vote_weight: u32,
}

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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LockStatus {
    ///
    pub block: Hash,
    ///
    pub round: u64,
    ///
    pub votes: Vec<Vote>,
}

///
pub trait ConsensusSupport {
    ///
    fn transmit(&self, msg: BftMsg) -> Result<(), ConsensusErr>;
    ///
    fn check_block(&self, block: &[u8]) -> Result<(), ConsensusErr>;
    ///
    fn commit(&self, commit: Commit) -> Result<(), ConsensusErr>;
}
