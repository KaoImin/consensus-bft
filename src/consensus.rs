use crate::*;
use crate::{collection::*, error::ConsensusError, wal::Wal};

use bft::{
    actuator::BftActuator as BFT, BftMsg, Proposal as BftProposal, Status as BftStatus,
    Vote as BftVote,
};
use blake2b::blake2b as blake2b_hash;
use crossbeam::crossbeam_channel::{select, unbounded, Receiver, Sender};

use std::collections::HashMap;
use std::thread;

///
pub type ConsensusResult<T> = Result<T, ConsensusError>;

pub(crate) const INIT_HEIGHT: u64 = 0;
const LOG_TYPE_SIGNED_PROPOSAL: u8 = 1;
const LOG_TYPE_RAW_BYTES: u8 = 2;
const LOG_TYPE_RICH_STATUS: u8 = 3;
const LOG_TYPE_BLOCK_TXS: u8 = 4;
const LOG_TYPE_VERIFY_BLOCK_PESP: u8 = 5;
const LOG_TYPE_PROPOSAL: u8 = 6;
const LOG_TYPE_VOTE: u8 = 7;
const LOG_TYPE_COMMIT: u8 = 8;

///
pub struct ConsensusExecutor(Sender<ConsensusInput>);

impl ConsensusExecutor {
    ///
    pub fn new<T: ConsensusSupport + Send + 'static>(
        support: T,
        address: Address,
        wal_path: String,
    ) -> Self {
        let (send, recv) = unbounded();
        Consensus::start(support, address, recv, wal_path);
        ConsensusExecutor(send)
    }

    ///
    pub fn send(&self, input: ConsensusInput) -> Result<(), ConsensusError> {
        self.0
            .send(input)
            .map_err(|_| ConsensusError::BlockVerifyDiff)
    }
}

///
pub struct Consensus<T: ConsensusSupport + Send + 'static> {
    bft_recv: Receiver<BftMsg>,
    out_recv: Receiver<ConsensusInput>,

    height: u64,
    bft: BFT,
    block: Option<Vec<u8>>,
    verified_block: Vec<Vec<u8>>,
    authority: AuthorityManage,
    votes: VoteCollector,
    proposals: ProposalCollector,
    proof: Option<Proof>,
    pre_hash: Option<Hash>,
    wal_log: Wal,
    block_cache: HashMap<Hash, Vec<u8>>,
    proposal_cache: HashMap<u64, Vec<SignedProposal>>,
    vote_cache: HashMap<u64, Vec<SignedVote>>,

    function: T,
}

impl<T> Consensus<T>
where
    T: ConsensusSupport + Send + 'static,
{
    ///
    fn new(support: T, address: Address, recv: Receiver<ConsensusInput>, wal_path: String) -> Self {
        let (a, r) = BFT::start(address);
        Consensus {
            bft_recv: r,
            out_recv: recv,

            height: INIT_HEIGHT,
            bft: a,
            block: None,
            verified_block: Vec::new(),
            authority: AuthorityManage::default(),
            votes: VoteCollector::new(),
            proposals: ProposalCollector::new(),
            proof: None,
            pre_hash: None,
            wal_log: Wal::new(wal_path).unwrap(),
            block_cache: HashMap::new(),
            proposal_cache: HashMap::new(),
            vote_cache: HashMap::new(),

            function: support,
        }
    }

    ///
    pub fn start(support: T, address: Address, recv: Receiver<ConsensusInput>, wal_path: String) {
        // self.load_wal_log();
        thread::spawn(move || {
            let mut engine = Consensus::new(support, address, recv, wal_path);
            loop {
                select! {
                    recv(engine.bft_recv) -> bft_msg => if let Ok(bft_msg) = bft_msg {
                    let _ = engine.inner_process(bft_msg);
                },
                    recv(engine.out_recv) -> out_msg => if let Ok(out_msg) = out_msg {
                    let _ = engine.outer_process(out_msg);
                },
                }
            }
        });
    }

    fn outer_process(&mut self, msg: ConsensusInput) -> Result<(), ConsensusError> {
        match msg {
            ConsensusInput::SignedProposal(sp) => {
                let (proposal, verify_resp) = self.handle_signed_proposal(sp, true)?;
                self.bft
                    .send_proposal(BftMsg::Proposal(proposal))
                    .expect("Send verify resp fail");
                if let Some(res) = verify_resp {
                    self.bft
                        .send_verify(BftMsg::VerifyResp(res.to_bft_resp()))
                        .expect("Send verify resp fail");
                }
                Ok(())
            }
            ConsensusInput::SignedVote(sv) => {
                let vote = self.handle_signed_vote(sv, true)?;
                self.bft
                    .send_vote(BftMsg::Vote(vote))
                    .expect("send bftvote fail");
                Ok(())
            }
            ConsensusInput::Status(rs) => {
                let status = self.handle_rich_status(rs, true)?;
                self.bft.send_status(BftMsg::Status(status)).unwrap();
                let height = self.height;
                self.send_cache_proposal(height)?;
                self.send_cache_vote(height)?;
                Ok(())
            }
            ConsensusInput::Feed(f) => {
                let block = f.block.clone();
                let height = f.height;
                if height < self.height {
                    return Err(ConsensusError::BlockVerifyDiff);
                } else if height == self.height {
                    let hash = blake2b_hash(&block);
                    let hash = hash.as_bytes();
                    self.block_cache
                        .entry((hash).to_vec())
                        .or_insert(f.clone().block);
                    self.bft
                        .send_feed(BftMsg::Feed(f.to_bft_feed(hash.to_vec())))
                        .unwrap();
                }
                Ok(())
            }
            ConsensusInput::Start => {
                self.bft
                    .send_start(BftMsg::Start)
                    .expect("send command fail");
                Ok(())
            }
            ConsensusInput::Pause => {
                self.bft
                    .send_start(BftMsg::Pause)
                    .expect("send command fail");
                Ok(())
            }
            _ => panic!(""),
        }
    }

    fn inner_process(&mut self, msg: BftMsg) -> Result<(), ConsensusError> {
        match msg {
            BftMsg::Proposal(p) => {
                let sp = self.handle_proposal(p, true)?;
                self.function
                    .transmit(ConsensusOutput::SignedProposal(sp))?;
                Ok(())
            }
            BftMsg::Vote(v) => {
                let sv = self.handle_vote(v, true)?;
                self.function.transmit(ConsensusOutput::SignedVote(sv))?;
                Ok(())
            }
            BftMsg::Commit(c) => {
                let commit = self.handle_commit(c, true)?;
                self.function.commit(commit)?;
                Ok(())
            }
            _ => panic!("BFT Core Error!"),
        }
    }

    fn send_cache_proposal(&mut self, height: u64) -> ConsensusResult<()> {
        let proposals = self.proposal_cache.get(&height);
        if proposals.is_none() {
            return Ok(());
        }
        for signed_proposal in proposals.unwrap().to_vec().into_iter() {
            let (proposal, resp) = self.handle_signed_proposal(signed_proposal, true)?;
            info!(
                "Cita-bft hands over bft_proposal to bft-rs!\n{:?}",
                proposal
            );
            self.bft
                .send_proposal(BftMsg::Proposal(proposal))
                .expect("send cache proposal fail");
            if let Some(result) = resp {
                self.bft
                    .send_verify(BftMsg::VerifyResp(result.to_bft_resp()))
                    .expect("send cache proposal fail");
            }
        }
        Ok(())
    }

    fn send_cache_vote(&mut self, height: u64) -> ConsensusResult<()> {
        let votes = self.vote_cache.get(&height);
        if votes.is_none() {
            return Ok(());
        }
        for signed_vote in votes.unwrap().to_vec().into_iter() {
            if let Ok(vote) = self.handle_signed_vote(signed_vote, true) {
                self.bft
                    .send_vote(BftMsg::Vote(vote))
                    .expect("send bftvote fail");
            }
        }
        Ok(())
    }

    fn handle_proposal(
        &mut self,
        proposal: BftProposal,
        need_wal: bool,
    ) -> ConsensusResult<SignedProposal> {
        let height = proposal.height;
        let round = proposal.round;
        if height < self.height {
            warn!(
                "The height of bft_proposal is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        if need_wal {
            let msg = proposal.to_json();
            if self.wal_log.save(height, LOG_TYPE_PROPOSAL, msg).is_err() {
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }
        let signed_proposal = self.build_signed_proposal(proposal)?;
        self.proposals.add(height, round, &signed_proposal);
        Ok(signed_proposal)
    }

    fn build_signed_proposal(&mut self, proposal: BftProposal) -> ConsensusResult<SignedProposal> {
        if self.proof.is_none() {
            return Err(ConsensusError::BlockVerifyDiff);
        }
        let height = proposal.height;
        let round = proposal.round;
        let hash = proposal.content;
        let lock_round = proposal.lock_round;

        let lock_votes = if let Some(res) = proposal.lock_round {
            let mut tmp = Vec::new();
            let vote_set = self
                .votes
                .get_vote_set(height, res, VoteType::Prevote)
                .expect("Build SignedVote Error!");
            for vote in proposal.lock_votes.unwrap().into_iter() {
                let v = vote_set
                    .vote_pair
                    .get(&vote)
                    .expect("Obsolete SignedVote in Step Propose!");
                tmp.push(v.to_owned());
            }
            tmp
        } else {
            Vec::new()
        };

        let block = self.block_cache.get(&hash).unwrap().to_owned();
        let signed_proposal = Proposal {
            height,
            round,
            block,
            proof: self.proof.clone().unwrap(),
            lock_round,
            lock_votes,
            proposer: proposal.proposer,
        };
        // sig
        let hash = self.function.crypt_hash(&signed_proposal.rlp_bytes());
        let sig = self.function.signature(&hash);
        if sig.is_none() {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        Ok(SignedProposal {
            proposal: signed_proposal,
            signature: sig.unwrap(),
        })
    }

    fn handle_vote(&mut self, vote: BftVote, need_wal: bool) -> ConsensusResult<SignedVote> {
        let height = vote.height;
        if height < self.height {
            warn!(
                "The height of bft_vote is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        if need_wal {
            let msg = vote.to_json();
            if self.wal_log.save(height, LOG_TYPE_VOTE, msg).is_err() {
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }
        let vtype = if vote.vote_type == bft::VoteType::Prevote {
            VoteType::Prevote
        } else {
            VoteType::Precommit
        };

        let signed_vote = Vote::from_bft_vote(vote.clone(), vtype.clone());
        let hash = self.function.crypt_hash(&signed_vote.rlp_bytes());
        if let Some(sig) = self.function.signature(&hash) {
            let res = SignedVote {
                vote: signed_vote,
                signature: sig,
            };
            self.votes.add(height, vote.round, vtype, &vote, &res);
            Ok(res)
        } else {
            return Err(ConsensusError::BlockVerifyDiff);
        }
    }

    fn handle_commit(&mut self, commit: bft::Commit, need_wal: bool) -> ConsensusResult<Commit> {
        let height = commit.height;
        if height < self.height {
            warn!(
                "The height of bft_commit is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        if need_wal {
            let msg = commit.to_json();
            if self.wal_log.save(height, LOG_TYPE_COMMIT, msg).is_err() {
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }

        let round = commit.clone().round;
        let block_hash = commit.clone().proposal;
        let vote = commit.clone().lock_votes;
        let proposal = commit.clone().proposal;
        let proof = self.generate_proof(height, round, block_hash, vote)?;
        let block = self
            .block_cache
            .get(&proposal)
            .expect("Obsolete Origin Block!")
            .to_owned();

        let res = Commit {
            height,
            pre_hash: self.pre_hash.clone().expect("Obsolete PrevHash!"),
            block,
            address: commit.address,
            proof,
        };

        Ok(res)
    }

    fn generate_proof(
        &mut self,
        height: u64,
        round: u64,
        block_hash: Hash,
        vote: Vec<bft::Vote>,
    ) -> ConsensusResult<Proof> {
        let mut tmp = HashMap::new();
        if let Some(vote_set) = self.votes.get_vote_set(height, round, VoteType::Precommit) {
            for v in vote.iter() {
                if let Some(signed_vote) = vote_set.vote_pair.get(&v) {
                    tmp.insert(v.clone().voter, signed_vote.signature.clone());
                } else {
                    return Err(ConsensusError::BlockVerifyDiff);
                }
            }
        } else {
            return Err(ConsensusError::BlockVerifyDiff);
        }
        let res = Proof {
            height,
            round,
            block_hash,
            precommit_votes: tmp,
        };
        Ok(res)
    }

    fn handle_signed_proposal(
        &mut self,
        msg: SignedProposal,
        need_wal: bool,
    ) -> ConsensusResult<(BftProposal, Option<VerifyResp>)> {
        //
        let sig = msg.signature.clone();
        let proposal = msg.proposal.clone();

        let hash = self.function.crypt_hash(&proposal.rlp_bytes());
        let address = self.function.check_signature(&sig, &hash);
        if address.is_none() {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let height = proposal.height;
        let round = proposal.round;

        if proposal.proposer != address.clone().unwrap() {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        if height < self.height - 1 {
            warn!(
                "The height of signed_proposal is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        let verify_resp = self.function.check_block(&proposal.block)?;

        if height >= self.height {
            if height - self.height < collection::CACHE_NUMBER as u64 {
                if need_wal {
                    let res = msg.to_json();
                    if self
                        .wal_log
                        .save(height, LOG_TYPE_SIGNED_PROPOSAL, res)
                        .is_err()
                    {
                        return Err(ConsensusError::BlockVerifyDiff);
                    }
                }
            }
            if height > self.height {
                self.proposal_cache
                    .entry(height)
                    .or_insert(vec![])
                    .push(msg);
                warn!(
                    "The height of signed_proposal is {} which is higher than self.height {}!",
                    height, self.height
                );
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }
        self.proposals.add(height, round, &msg);
        let hash = proposal.block.clone();
        let bft_proposal = proposal.to_bft_proposal();

        self.check_proposer(height, round, &address.unwrap())?;
        self.check_lock_votes(&msg, hash.clone())?;

        let verify_resp = VerifyResp {
            is_pass: verify_resp,
            block_hash: bft_proposal.content.clone(),
        };

        if height < self.height {
            return Ok((bft_proposal, Some(verify_resp)));
        }

        if self.verified_block.contains(&hash) {
            return Ok((bft_proposal, Some(verify_resp)));
        }

        self.check_proof(height, &proposal.proof)?;

        self.function
            .check_block(&self.block_cache.get(&hash).expect("Lost origin block"))?;
        Ok((bft_proposal, None))
    }

    fn handle_signed_vote(&mut self, msg: SignedVote, need_wal: bool) -> ConsensusResult<BftVote> {
        let sig = msg.signature.clone();
        let vote = msg.vote.clone();

        let hash = self.function.crypt_hash(&msg.rlp_bytes());
        let address = self.function.check_signature(&sig, &hash);
        if address.is_none() {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let height = vote.height.clone();
        let round = vote.round.clone();
        let sender = vote.voter.clone();
        if height < self.height - 1 {
            warn!(
                "The height of raw_bytes is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        let address = address.unwrap();
        if sender != address {
            error!("The address recovers from the signature is {:?} which is mismatching with the sender {:?}!", &address, &sender);
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let bft_vote = vote.to_bft_vote();

        if height >= self.height {
            if height - self.height < collection::CACHE_NUMBER as u64 {
                if need_wal {
                    let res = msg.to_json();
                    if self.wal_log.save(height, LOG_TYPE_RAW_BYTES, res).is_err() {
                        return Err(ConsensusError::BlockVerifyDiff);
                    }
                }
            }
            if height > self.height {
                self.vote_cache.entry(height).or_insert(vec![]).push(msg);
                warn!(
                    "The height of raw_bytes is {} which is higher than self.height {}!",
                    height, self.height
                );
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }
        self.votes
            .add(height, round, vote.vote_type, &bft_vote, &msg);
        self.check_vote_sender(height, &sender)?;
        Ok(bft_vote)
    }

    fn handle_rich_status(&mut self, msg: Status, need_wal: bool) -> ConsensusResult<BftStatus> {
        let rich_status = msg;
        let height = rich_status.height;
        if height < self.height {
            warn!(
                "The height of rich_status is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }
        if need_wal {
            let msg = rich_status.to_json();
            if self
                .wal_log
                .save(height, LOG_TYPE_RICH_STATUS, msg)
                .is_err()
            {
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }
        let pre_hash = rich_status.clone().pre_hash;
        self.pre_hash = Some(pre_hash);

        self.authority
            .update_authority(self.height, rich_status.clone().authority_list);

        self.goto_new_height(height)?;
        let bft_status = rich_status.to_bft_status();
        Ok(bft_status)
    }

    fn check_proposer(&self, height: u64, round: u64, address: &[u8]) -> ConsensusResult<()> {
        if height < self.height - 1 {
            error!(
                "The height {} is less than self.height {} - 1, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let mut authorities = &self.authority.authorities;
        if height == self.authority.authority_h_old {
            info!("Cita-bft sets the authority manage with old authorities!");
            authorities = &self.authority.authorities_old;
        }
        if (*authorities).is_empty() {
            error!("The size of authority manage is empty!");
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let mut auth_list = Vec::new();
        for node in authorities.iter() {
            auth_list.push(node.clone().address);
        }

        let proposer_nonce = height + round;
        let proposer = &auth_list[proposer_nonce as usize % (*authorities).len()];

        if *proposer == address.to_vec() {
            Ok(())
        } else {
            error!(
                "The proposer is invalid, while the rightful proposer is {:?}",
                proposer
            );
            Err(ConsensusError::BlockVerifyDiff)
        }
    }

    fn check_lock_votes(
        &mut self,
        signed_proposal: &SignedProposal,
        proposal_hash: Hash,
    ) -> ConsensusResult<()> {
        let proposal = &signed_proposal.proposal;
        let height = proposal.height;
        if height < self.height - 1 {
            error!(
                "The height {} is less than self.height {} - 1, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let mut map = HashMap::new();
        if let Some(lock_round) = proposal.lock_round {
            let votes = proposal.lock_votes.clone();
            for vote in votes.into_iter() {
                let sender =
                    self.check_signed_vote(height, lock_round, proposal_hash.clone(), vote)?;
                if let Some(_) = map.insert(sender, 1) {
                    return Err(ConsensusError::BlockVerifyDiff);
                }
            }
        } else {
            return Ok(());
        }

        let mut authority_n = &self.authority.authorities;
        if height == self.authority.authority_h_old {
            authority_n = &self.authority.authorities_old;
        }

        if map.len() * 3 > authority_n.len() * 2 {
            return Ok(());
        }
        Err(ConsensusError::BlockVerifyDiff)
    }

    fn check_signed_vote(
        &mut self,
        height: u64,
        round: u64,
        proposal_hash: Hash,
        signed_vote: SignedVote,
    ) -> ConsensusResult<Address> {
        let vote = signed_vote.vote.clone();
        if height < self.height - 1 {
            error!(
                "The vote's height {} is less than self.height {} - 1, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let mut authorities = &self.authority.authorities;
        if height == self.authority.authority_h_old {
            info!("Cita-bft sets the authority manage with old authorities!");
            authorities = &self.authority.authorities_old;
        }

        let hash = vote.block_hash.clone();
        if hash != proposal_hash {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let sender = vote.voter.clone();
        if !authorities.contains(&Node {
            address: sender.clone(),
            proposal_weight: 1,
            vote_weight: 1,
        }) {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let sig = signed_vote.signature.clone();
        let hash = self.function.crypt_hash(&vote.rlp_bytes());

        let address = self.function.check_signature(&sig, &hash);
        if address.is_none() {
            return Err(ConsensusError::BlockVerifyDiff);
        }
        if address.clone().unwrap() != sender {
            error!("The address recovers from the signature is {:?} which is mismatching with the sender {:?}!",
                &address,
                &sender
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let bft_vote = vote.to_bft_vote();
        self.votes
            .add(height, round, VoteType::Prevote, &bft_vote, &signed_vote);
        Ok(sender)
    }

    fn check_proof(&mut self, height: u64, proof: &Proof) -> ConsensusResult<()> {
        if height != self.height {
            error!(
                "The height {} is less than self.height {}, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        if self.authority.authority_h_old == height - 1 {
            if !self.verify_proof(&proof, height - 1, &self.authority.authorities_old) {
                error!("The proof of the block verified failed with old authorities!");
                return Err(ConsensusError::BlockVerifyDiff);
            }
        } else if !self.verify_proof(&proof, height - 1, &self.authority.authorities) {
            error!("The proof of the block verified failed with newest authorities!");
            return Err(ConsensusError::BlockVerifyDiff);
        }
        let proof = proof.to_owned();

        if let Some(res) = self.proof.clone() {
            if res.height != height - 1 {
                self.proof = Some(proof.clone());
            }
        } else {
            self.proof = Some(proof);
        }
        Ok(())
    }

    fn check_vote_sender(&self, height: u64, sender: &Address) -> ConsensusResult<()> {
        if height < self.height - 1 {
            error!(
                "The height {} is less than self.height {} - 1, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let mut authorities = &self.authority.authorities;
        if height == self.authority.authority_h_old {
            info!("Cita-bft sets the authority manage with old authorities!");
            authorities = &self.authority.authorities_old;
        }
        if !authorities.contains(&Node {
            address: sender.to_owned(),
            proposal_weight: 1,
            vote_weight: 1,
        }) {
            error!("The raw_bytes have invalid voter {:?}!", &sender);
            return Err(ConsensusError::BlockVerifyDiff);
        }
        Ok(())
    }

    fn goto_new_height(&mut self, height: u64) -> ConsensusResult<()> {
        self.verified_block.clear();
        self.block = None;
        self.height = height + 1;
        if let Err(_) = self.wal_log.set_height(self.height) {
            error!("Wal log set height {} failed!", self.height);
            return Err(ConsensusError::BlockVerifyDiff);
        };
        Ok(())
    }

    fn verify_proof(&self, proof: &Proof, h: u64, authorities: &[Node]) -> bool {
        if h == 0 {
            return true;
        }
        if h != proof.height {
            return false;
        }
        if 2 * authorities.len() >= 3 * proof.precommit_votes.len() {
            return false;
        }

        for (sender, sig) in proof.precommit_votes.iter() {
            if authorities.contains(&Node {
                address: sender.clone(),
                proposal_weight: 1,
                vote_weight: 1,
            }) {
                let msg = Vote {
                    vote_type: VoteType::Precommit,
                    height: proof.height,
                    round: proof.round,
                    block_hash: proof.block_hash.clone(),
                    voter: sender.clone(),
                };
                let hash = self.function.crypt_hash(&msg.rlp_bytes());
                if let Some(address) = self.function.check_signature(&sig, &hash) {
                    if &address != sender {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }
        true
    }

    // fn load_wal_log(&mut self) {
    //     info!("Cita-bft starts to load wal log!");
    //     let vec_buf = self.wal_log.load();
    //     for (msg_type, msg) in vec_buf {
    //         match msg_type {
    //             LOG_TYPE_SIGNED_PROPOSAL => {
    //                 info!("Cita-bft loads signed_proposal!");
    //                 let msg = Message::try_from(msg).expect("Try from message failed!");
    //                 if let Ok((proposal, verify_resp)) = self.handle_signed_proposal(msg, false) {
    //                     info!(
    //                         "Cita-bft hands over bft_proposal to bft-rs!\n{:?}",
    //                         proposal
    //                     );
    //                     self.cita2bft
    //                         .send(BftMsg::Proposal(proposal))
    //                         .expect("Cita-bft hands over bft_proposal failed!");
    //                     if let Some(verify_resp) = verify_resp {
    //                         info!(
    //                             "Cita-bft hands over verify_resp to bft-rs!\n{:?}",
    //                             verify_resp
    //                         );
    //                         self.cita2bft
    //                             .send(BftMsg::VerifyResp(verify_resp))
    //                             .expect("Cita-bft hands over verify_resp failed!");
    //                     }
    //                 };
    //             }
    //             LOG_TYPE_RAW_BYTES => {
    //                 info!("Cita-bft loads raw_bytes message!");
    //                 let msg = Message::try_from(msg).expect("Try from message failed!");
    //                 if let Ok(vote) = self.handle_raw_bytes(msg, false) {
    //                     info!("Cita-bft hands over bft_vote to bft-rs!\n{:?}", vote);
    //                     self.cita2bft
    //                         .send(BftMsg::Vote(vote))
    //                         .expect("Cita-bft hands over bft_vote failed!");
    //                 };
    //             }
    //             LOG_TYPE_RICH_STATUS => {
    //                 info!("Cita-bft loads rich_status message!");
    //                 let msg = Message::try_from(msg).expect("Try from message failed!");
    //                 if let Ok(status) = self.handle_rich_status(msg, false) {
    //                     info!("Cita-bft hands over bft_status to bft-rs!\n{:?}", status);
    //                     self.cita2bft
    //                         .send(BftMsg::Status(status))
    //                         .expect("Cita-bft hands over bft_status failed!");
    //                 };
    //             }
    //             LOG_TYPE_BLOCK_TXS => {
    //                 info!("Cita-bft loads block_txs message!");
    //                 let msg = Message::try_from(msg).expect("Try from message failed!");
    //                 if let Ok(feed) = self.handle_block_txs(msg, false) {
    //                     info!("Cita-bft hands over bft_feed to bft-rs!\n{:?}", feed);
    //                     self.cita2bft
    //                         .send(BftMsg::Feed(feed))
    //                         .expect("Cita-bft hands over bft_feed failed!");
    //                 };
    //             }
    //             LOG_TYPE_VERIFY_BLOCK_PESP => {
    //                 info!("Cita-bft loads verify_block_resp message!");
    //                 let msg = Message::try_from(msg).expect("Try from message failed!");
    //                 if let Ok(verify_resp) = self.handle_verify_block_resp(msg, false) {
    //                     info!(
    //                         "Cita-bft hands over verify_resp to bft-rs!\n{:?}",
    //                         verify_resp
    //                     );
    //                     self.cita2bft
    //                         .send(BftMsg::VerifyResp(verify_resp))
    //                         .expect("Cita-bft hands over verify_resp failed!");
    //                 };
    //             }
    //             LOG_TYPE_PROPOSAL => {
    //                 info!("Cita-bft loads bft_proposal message!");
    //                 let proposal: BftProposal =
    //                     deserialize(&msg[..]).expect("Deserialize message failed!");
    //                 if let Ok(signed_proposal) = self.handle_proposal(proposal.clone(), false) {
    //                     info!(
    //                         "Cita-bft sends signed_proposal to rabbit_mq!\n{:?}",
    //                         proposal
    //                     );
    //                     let msg: Message = signed_proposal.into();
    //                     self.cita2rab
    //                         .send((
    //                             routing_key!(Consensus >> SignedProposal).into(),
    //                             msg.try_into().expect("Try into message failed!"),
    //                         ))
    //                         .expect("Cita-bft sends signed_proposal failed!");;
    //                 };
    //             }
    //             LOG_TYPE_VOTE => {
    //                 info!("Cita-bft loads bft_vote message!");
    //                 let vote: BftVote = deserialize(&msg[..]).expect("Deserialize message failed!");
    //                 if let Ok(raw_bytes) = self.handle_vote(vote.clone(), false) {
    //                     info!("Cita-bft sends raw_bytes to rabbit_mq!\n{:?}", vote);
    //                     let msg: Message = raw_bytes.into();
    //                     self.cita2rab
    //                         .send((
    //                             routing_key!(Consensus >> RawBytes).into(),
    //                             msg.try_into().expect("Try into message failed!"),
    //                         ))
    //                         .expect("Cita-bft sends raw_bytes failed!");
    //                 };
    //             }
    //             LOG_TYPE_COMMIT => {
    //                 info!("Cita-bft loads bft_commit message!");
    //                 let commit: Commit =
    //                     deserialize(&msg[..]).expect("Deserialize message failed!");
    //                 if let Ok(block_with_proof) = self.handle_commit(commit.clone(), true) {
    //                     info!(
    //                         "Cita-bft sends block_with_proof to rabbit_mq!\n{:?}",
    //                         commit
    //                     );
    //                     let msg: Message = block_with_proof.into();
    //                     self.cita2rab
    //                         .send((
    //                             routing_key!(Consensus >> BlockWithProof).into(),
    //                             msg.try_into().expect("Try into message failed!"),
    //                         ))
    //                         .expect("Cita-bft sends block_with_proof failed!");
    //                 };
    //             }
    //             _ => {}
    //         }
    //     }
    //     info!("Cita-bft successfully processes the whole wal log!");
    // }
}

#[inline(always)]
pub(crate) fn safe_unwrap_result<T, E>(
    result: Result<T, E>,
    err: ConsensusError,
) -> ConsensusResult<T> {
    if let Ok(value) = result {
        return Ok(value);
    }
    Err(err)
}

#[inline(always)]
pub(crate) fn safe_unwrap_option<T>(option: Option<T>, err: ConsensusError) -> ConsensusResult<T> {
    if let Some(value) = option {
        return Ok(value);
    }
    Err(err)
}

///
pub fn check_proof(
    proof: &Proof,
    h: u64,
    authorities: &[Node],
    crypt_hash: fn(msg: &[u8]) -> Vec<u8>,
    check_signature: fn(signature: &[u8], hash: &[u8]) -> Option<Address>,
) -> bool {
    if h == 0 {
        return true;
    }
    if h != proof.height {
        return false;
    }
    if 2 * authorities.len() >= 3 * proof.precommit_votes.len() {
        return false;
    }
    for (sender, sig) in proof.precommit_votes.iter() {
        if authorities.contains(&Node {
            address: sender.clone(),
            proposal_weight: 1,
            vote_weight: 1,
        }) {
            let msg = Vote {
                vote_type: VoteType::Precommit,
                height: proof.height,
                round: proof.round,
                block_hash: proof.block_hash.clone(),
                voter: sender.clone(),
            };
            let hash = crypt_hash(&msg.rlp_bytes());
            if let Some(address) = check_signature(&sig, &hash) {
                if &address != sender {
                    return false;
                }
            }
        } else {
            return false;
        }
    }
    true
}
