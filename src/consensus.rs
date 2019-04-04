use crate::*;
use crate::{
    error::ConsensusError,
    wal::Wal,
};

use bft_rs::{actuator::BftActuator as BFT, *};
use std::collections::HashMap;

pub(crate) const INIT_HEIGHT = 0;

///
pub struct Consensus<T: ConsensusSupport> {
    height: u64,
    bft: BFT,
    block: Option<Vec<u8>>,
    verified_block: Vec<Vec<u8>>,
    authority: AuthorityManage,
    proof: Option<Proof>,
    pre_hash: Option<Hash>,
    wal_log: Wal,
    proposal_cache: HashMap<u64, Proposal>,
    vote_cache: HashMap<u64, Vote>,

    function: T,
}

impl<T> for Consensus<T>
where T: ConsensusSupport
{
    ///
    pub fn new(support: T, address: Address, wal_path: Path) -> Self {
        Consensus {
            height: INIT_HEIGHT,
            bft: BFT::new(address),
            block: None,
            verified_block: Vec::new();
            authority: AuthorityManage::default(),
            proof: None,
            pre_hash: None,
            wal: Wal::create(wal_path),
            proposal_cache: HashMap::new();
            vote_cache: HashMap::new();
            function: support,
        }
    }

    ///
    pub fn send(&mut self, input: BftInput) -> Result<(), ConsensusError> {
        match input {
            Proposal(p) => {}
            Vote(v) => {}
            Status(s) => {}
            VerifyResp(r) => {}
            Feed(f) => {}
            Pause => self.bft.send_cmd(BftMsg::Pause).expect(""),
            Start => self.bft.send_cmd(BftMsg::Start).expect(""),
        }
    }
}
