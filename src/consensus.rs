use crate::*;
use crate::{error::ConsensusError, wal::Wal};

use bft_rs::{actuator::BftActuator as BFT, BftMsg};
use std::collections::HashMap;

pub(crate) const INIT_HEIGHT: u64 = 0;

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
    proposal_cache: HashMap<u64, SignedProposal>,
    vote_cache: HashMap<u64, SignedVote>,
    verify_result: HashMap<Hash, bool>,

    function: T,
}

impl<T> Consensus<T>
where
    T: ConsensusSupport,
{
    ///
    pub fn new(support: T, address: Address, wal_path: Path) -> Self {
        Consensus {
            height: INIT_HEIGHT,
            bft: BFT::new(address),
            block: None,
            verified_block: Vec::new(),
            authority: AuthorityManage::default(),
            proof: None,
            pre_hash: None,
            wal_log: Wal::create(wal_path).unwrap(),
            proposal_cache: HashMap::new(),
            vote_cache: HashMap::new(),
            verify_result: HashMap::new(),

            function: support,
        }
    }

    ///
    pub fn send(&mut self, input: ConsensusInput) -> Result<(), ConsensusError> {
        match input {
            ConsensusInput::SignedProposal(p) => {
                self.bft.send_proposal(p.to_bft_proposal()).map_err(|_| ConsensusError::SendMsgErr)
            }
            ConsensusInput::SignedVote(v) => {
                self.bft.send_vote(v.to_bft_vote()).map_err(|_| ConsensusError::SendMsgErr)
            }
            ConsensusInput::Status(s) => {
                self.bft.send_status(s.to_bft_status()).map_err(|_| ConsensusError::SendMsgErr)
            }
            ConsensusInput::VerifyResp(r) => {
                if let Some(res) = self.verify_result.get(&r.block_hash) {
                    if *res != r.is_pass {
                        return Err(ConsensusError::BlockVerifyDiff);
                    } else {
                        return Ok(());
                    }
                } else {
                    self.verify_result.entry(r.block_hash).or_insert(r.is_pass);
                    return Ok(());
                }
            }
            ConsensusInput::Feed(f) => {
                self.block = Some(f.block);
                return Ok(());
            }
            ConsensusInput::Pause => self.bft.send_command(BftMsg::Pause).map_err(|_| ConsensusError::SendMsgErr),
            ConsensusInput::Start => self.bft.send_command(BftMsg::Start).map_err(|_| ConsensusError::SendMsgErr),
        }
    }
}
