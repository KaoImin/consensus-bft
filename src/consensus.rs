use crate::{
    collection::*,
    error::ConsensusError,
    types::*,
    util::{into_addr_set, turbo_hash, SendMsg},
    wal::Wal,
    ConsensusSupport, Content,
};

use bft_core::types::{
    Commit as BftCommit, CoreInput, CoreOutput, Feed as BftFeed, Proposal as BftProposal,
    Status as BftStatus, VerifyResp as BftVerifyResp, Vote as BftVote, VoteType as BftVoteType,
};
use bft_core::Core as BFT;
use crossbeam_channel::{select, unbounded, Receiver, Sender};
use crossbeam_utils::thread as crossbeam_thread;
use log::{error, info, warn};
use rlp::Encodable;
use serde_json::{from_slice, to_string};
use std::{
    collections::{HashMap, HashSet},
    thread,
};

/// Consensus result.
pub type Result<T> = ::std::result::Result<T, ConsensusError>;

pub(crate) const INIT_HEIGHT: u64 = 0;
const LOG_TYPE_SIGNED_PROPOSAL: u8 = 1;
const LOG_TYPE_RAW_BYTES: u8 = 2;
const LOG_TYPE_RICH_STATUS: u8 = 3;
const LOG_TYPE_BLOCK_TXS: u8 = 4;
const LOG_TYPE_VERIFY_BLOCK_PESP: u8 = 5;
const LOG_TYPE_PROPOSAL: u8 = 6;
const LOG_TYPE_VOTE: u8 = 7;
const LOG_TYPE_COMMIT: u8 = 8;

/// A consensus executor.
#[derive(Clone, Debug)]
pub struct ConsensusExecutor<F: Content + Sync>(Sender<ConsensusInput<F>>);

impl<F> ConsensusExecutor<F>
where
    F: Content + Sync,
{
    /// A function to generate a new consensus executor.
    pub fn new<T: ConsensusSupport<F> + Send + 'static + Clone + Sync>(
        support: T,
        address: Address,
        wal_path: &str,
    ) -> Self {
        let (send, recv) = unbounded();
        Consensus::start(support, address, recv, wal_path.to_string());
        ConsensusExecutor(send)
    }

    /// A functiont to send a `ConsensusInput`.
    pub fn send(&self, input: ConsensusInput<F>) -> Result<()> {
        self.0.send(input).map_err(|_| ConsensusError::SendMsgErr)
    }
}

/// A consensus type.
pub(crate) struct Consensus<
    T: ConsensusSupport<F> + Send + 'static + Sync + Clone,
    F: Content + Sync,
> {
    bft_recv: Receiver<CoreOutput>,
    interface_recv: Receiver<ConsensusInput<F>>,
    async_send: Sender<AsyncMsg<F>>,
    async_recv: Receiver<AsyncMsg<F>>,

    height: u64,
    bft: BFT,
    block: Option<Vec<u8>>,
    verified_block: HashMap<Hash, bool>,
    address: Vec<u8>,
    authority: AuthorityManage,
    votes: VoteCollector,
    proposals: ProposalCollector<F>,
    proof: Option<Proof<F>>,
    prev_hash: Option<Hash>,
    wal_log: Wal,
    block_cache: HashMap<Hash, F>,
    proposal_cache: HashMap<u64, Vec<SignedProposal<F>>>,
    vote_cache: HashMap<u64, Vec<SignedVote>>,
    consensus_power: bool,

    function: T,
}

impl<T, F> Consensus<T, F>
where
    T: ConsensusSupport<F> + Send + 'static + Sync + Clone,
    F: Content + Sync,
{
    fn new(
        support: T,
        address: Address,
        recv: Receiver<ConsensusInput<F>>,
        wal_path: String,
    ) -> Self {
        let (send, bft_recv) = unbounded();
        let core = BFT::new(SendMsg::new(send), address.clone());
        let (async_send, async_recv) = unbounded();
        Consensus {
            bft_recv,
            interface_recv: recv,
            async_send,
            async_recv,

            height: INIT_HEIGHT,
            bft: core,
            block: None,
            verified_block: HashMap::new(),
            address,
            authority: AuthorityManage::default(),
            votes: VoteCollector::new(),
            proposals: ProposalCollector::new(),
            proof: None,
            prev_hash: None,
            wal_log: Wal::new(wal_path).unwrap(),
            block_cache: HashMap::new(),
            proposal_cache: HashMap::new(),
            vote_cache: HashMap::new(),
            consensus_power: false,

            function: support,
        }
    }

    /// A function to start a consensus service.
    pub(crate) fn start(
        support: T,
        address: Address,
        recv: Receiver<ConsensusInput<F>>,
        wal_path: String,
    ) {
        // self.load_wal_log();
        thread::spawn(move || {
            let mut engine = Consensus::new(support, address, recv, wal_path);
            engine.load_wal_log();
            loop {
                select! {
                    recv(engine.bft_recv) -> bft_msg => if let Ok(bft_msg) = bft_msg {
                        let _ = engine.core_process(bft_msg).map_err(|err| panic!("Consensus Error {:?}", err));
                    },
                    recv(engine.interface_recv) -> external_msg => if let Ok(external_msg) = external_msg {
                        let _ = engine.external_process(external_msg).map_err(|err| panic!("Consensus Error {:?}", err));
                    },
                    recv(engine.async_recv) -> async_msg => if let Ok(async_msg) = async_msg {
                        let _ = engine.async_process(async_msg).map_err(|err| panic!("Consensus Error {:?}", err));
                    },
                }
            }
        });
    }

    fn async_process(&mut self, msg: AsyncMsg<F>) -> Result<()> {
        match msg {
            AsyncMsg::VerifyResp(vr) => {
                let hash = vr.proposal.clone();
                self.bft
                    .send_bft_msg(CoreInput::VerifyResp(BftVerifyResp {
                        is_pass: vr.is_pass,
                        proposal: hash.clone(),
                    }))
                    .map_err(|_| ConsensusError::SendMsgErr)?;
                self.verified_block.entry(hash).or_insert(vr.is_pass);

                if let Ok(res) = to_string(&vr) {
                    if self
                        .wal_log
                        .save(self.height, LOG_TYPE_VERIFY_BLOCK_PESP, res)
                        .is_err()
                    {
                        return Err(ConsensusError::SaveWalErr);
                    }
                } else {
                    return Err(ConsensusError::SerJsonErr);
                }
            }
            AsyncMsg::Feed(f) => {
                let hash = if cfg!(feature = "turbo_hash") {
                    self.function.hash(&turbo_hash(f.content.rlp_bytes()))
                } else {
                    self.function.hash(&f.content.rlp_bytes())
                };

                self.bft
                    .send_bft_msg(CoreInput::Feed(BftFeed {
                        height: f.height,
                        proposal: hash.clone(),
                    }))
                    .map_err(|_| ConsensusError::SendMsgErr)?;

                self.block_cache
                    .entry(hash)
                    .or_insert_with(|| f.content.clone());
                if let Ok(msg) = to_string(&f) {
                    if self
                        .wal_log
                        .save(self.height, LOG_TYPE_BLOCK_TXS, msg)
                        .is_err()
                    {
                        return Err(ConsensusError::SaveWalErr);
                    }
                } else {
                    return Err(ConsensusError::SerJsonErr);
                }
            }
        }
        Ok(())
    }

    fn external_process(&mut self, msg: ConsensusInput<F>) -> Result<()> {
        match msg {
            ConsensusInput::SignedProposal(sp) => {
                info!("Receive signed proposal");
                let (proposal, verify_resp) = self.handle_signed_proposal(sp, true)?;
                self.bft
                    .send_bft_msg(CoreInput::Proposal(proposal))
                    .map_err(|_| ConsensusError::SendMsgErr)?;
                if let Some(res) = verify_resp {
                    self.bft
                        .send_bft_msg(CoreInput::VerifyResp(res.to_bft_resp()))
                        .map_err(|_| ConsensusError::SendMsgErr)?;
                }
                Ok(())
            }
            ConsensusInput::SignedVote(sv) => {
                info!("Receive signed vote");
                let vote = self.handle_signed_vote(sv, true)?;
                self.bft
                    .send_bft_msg(CoreInput::Vote(vote))
                    .map_err(|_| ConsensusError::SendMsgErr)?;
                Ok(())
            }
            ConsensusInput::Status(rs) => {
                info!("Receive status");
                let status = self.handle_rich_status(rs.clone(), true)?;

                if into_addr_set(rs.authority_list).contains(&self.address) {
                    if !self.consensus_power {
                        self.consensus_power = true;
                        self.bft
                            .send_bft_msg(CoreInput::Start)
                            .map_err(|_| ConsensusError::SendMsgErr)?;
                    }
                } else if self.consensus_power {
                    self.consensus_power = false;
                    self.bft
                        .send_bft_msg(CoreInput::Pause)
                        .map_err(|_| ConsensusError::SendMsgErr)?;
                }

                self.bft
                    .send_bft_msg(CoreInput::Status(status))
                    .map_err(|_| ConsensusError::SendMsgErr)?;
                self.send_cache_proposal(self.height)?;
                self.send_cache_vote(self.height)?;
                Ok(())
            }
        }
    }

    fn core_process(&mut self, msg: CoreOutput) -> Result<()> {
        match msg {
            CoreOutput::Proposal(p) => {
                info!("Receive proposal");
                let sp = self.handle_proposal(p, true)?;
                self.function
                    .transmit(ConsensusOutput::SignedProposal(sp))
                    .map_err(|_| ConsensusError::SupportErr)
            }
            CoreOutput::Vote(v) => {
                info!("Receive vote");
                let sv = self.handle_vote(v, true)?;
                self.function
                    .transmit(ConsensusOutput::SignedVote(sv))
                    .map_err(|_| ConsensusError::SupportErr)
            }
            CoreOutput::Commit(c) => {
                info!("Receive commit");
                let commit = self.handle_commit(c, true)?;
                self.function
                    .commit(commit)
                    .map_err(|_| ConsensusError::SupportErr)
            }
            CoreOutput::GetProposalRequest(h) => {
                info!("Receive get proposal request");
                self.ask_for_proposal(h);
                Ok(())
            }
        }
    }

    fn send_cache_proposal(&mut self, height: u64) -> Result<()> {
        let proposals = self.proposal_cache.remove_entry(&height);
        if proposals.is_none() {
            return Ok(());
        }
        for signed_proposal in proposals.unwrap().1.into_iter() {
            let (proposal, resp) = self.handle_signed_proposal(signed_proposal, true)?;
            info!(
                "Consensus hands over bft_proposal to bft-rs!\n{:?}",
                proposal
            );
            self.bft
                .send_bft_msg(CoreInput::Proposal(proposal))
                .map_err(|_| ConsensusError::SendMsgErr)?;
            if let Some(result) = resp {
                self.bft
                    .send_bft_msg(CoreInput::VerifyResp(result.to_bft_resp()))
                    .map_err(|_| ConsensusError::SendMsgErr)?;
            }
        }
        Ok(())
    }

    fn send_cache_vote(&mut self, height: u64) -> Result<()> {
        let votes = self.vote_cache.remove_entry(&height);
        if votes.is_none() {
            return Ok(());
        }
        for signed_vote in votes.unwrap().1.into_iter() {
            let vote = self.handle_signed_vote(signed_vote, true)?;
            self.bft
                .send_bft_msg(CoreInput::Vote(vote))
                .map_err(|_| ConsensusError::SendMsgErr)?;
        }
        Ok(())
    }

    fn handle_proposal(
        &mut self,
        proposal: BftProposal,
        need_wal: bool,
    ) -> Result<SignedProposal<F>> {
        let height = proposal.height;
        let round = proposal.round;

        if height < self.height {
            error!(
                "The height of bft_proposal is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BftCoreErr);
        }

        if let Some(origin_proposal) = self.block_cache.get(&proposal.content) {
            self.verify_proposal(origin_proposal.to_owned());
        } else {
            return Err(ConsensusError::LoseBlock);
        }

        if need_wal {
            if let Ok(msg) = to_string(&proposal) {
                if self.wal_log.save(height, LOG_TYPE_PROPOSAL, msg).is_err() {
                    return Err(ConsensusError::SaveWalErr);
                }
            } else {
                return Err(ConsensusError::SerJsonErr);
            }
        }
        let signed_proposal = self.build_signed_proposal(proposal)?;
        self.proposals.add(height, round, &signed_proposal);
        Ok(signed_proposal)
    }

    fn build_signed_proposal(&mut self, proposal: BftProposal) -> Result<SignedProposal<F>> {
        if self.proof.is_none() && (self.height != 1) {
            return Err(ConsensusError::MissingProof);
        }

        let height = proposal.height;
        let round = proposal.round;
        let hash = proposal.content;
        let lock_round = proposal.lock_round;

        let lock_votes = if proposal.lock_round.is_some() {
            let mut res = Vec::new();
            let vote_set = self
                .votes
                .get_vote_set(height, round, VoteType::Prevote)
                .ok_or(0)
                .map_err(|_| ConsensusError::NoVoteset)?;
            for vote in proposal.lock_votes.into_iter() {
                if let Some(v) = vote_set.vote_pair.get(&vote) {
                    res.push(v.to_owned());
                } else {
                    return Err(ConsensusError::LoseSignedVote);
                }
            }
            res
        } else {
            Vec::new()
        };

        let content = if let Some(res) = self.block_cache.get(&hash) {
            res.to_owned()
        } else {
            return Err(ConsensusError::LoseBlock);
        };

        let proof = if self.height == 1 {
            Proof {
                height: 0,
                round: 0,
                block_hash: content.clone(),
                precommit_votes: HashMap::new(),
            }
        } else {
            self.proof.clone().unwrap()
        };

        let signed_proposal = Proposal {
            height,
            round,
            content,
            proof,
            lock_round,
            lock_votes,
            proposer: proposal.proposer,
        };
        // sig
        let hash = self.function.hash(&signed_proposal.rlp_bytes());
        let sig = self
            .function
            .sign(&hash)
            .map_err(|_| ConsensusError::SupportErr)?;

        Ok(SignedProposal {
            proposal: signed_proposal,
            signature: sig,
        })
    }

    fn handle_vote(&mut self, vote: BftVote, need_wal: bool) -> Result<SignedVote> {
        let height = vote.height;
        if height < self.height {
            warn!(
                "The height of bft_vote is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BftCoreErr);
        }

        if need_wal {
            if let Ok(msg) = to_string(&vote) {
                if self.wal_log.save(height, LOG_TYPE_VOTE, msg).is_err() {
                    return Err(ConsensusError::SaveWalErr);
                }
            } else {
                return Err(ConsensusError::SerJsonErr);
            }
        }

        let vtype = if vote.vote_type == BftVoteType::Prevote {
            VoteType::Prevote
        } else {
            VoteType::Precommit
        };

        let signed_vote = Vote::from_bft_vote(vote.clone(), vtype.clone());
        let hash = self.function.hash(&signed_vote.rlp_bytes());
        if let Ok(sig) = self.function.sign(&hash) {
            let res = SignedVote {
                vote: signed_vote,
                signature: sig,
            };
            self.votes.add(height, vote.round, vtype, &vote, &res);
            Ok(res)
        } else {
            return Err(ConsensusError::SupportErr);
        }
    }

    fn handle_commit(&mut self, commit: BftCommit, need_wal: bool) -> Result<Commit<F>> {
        let height = commit.height;
        if height < self.height {
            warn!(
                "The height of bft_commit is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::BftCoreErr);
        }

        if need_wal {
            if let Ok(msg) = to_string(&commit) {
                if self.wal_log.save(height, LOG_TYPE_COMMIT, msg).is_err() {
                    return Err(ConsensusError::SaveWalErr);
                }
            } else {
                return Err(ConsensusError::SerJsonErr);
            }
        }

        let round = commit.round;
        let vote = commit.lock_votes.clone();
        let proposal = commit.proposal;

        let proposal = self.block_cache.get(&proposal);
        if proposal.is_some() {
            let proposal = proposal.unwrap().to_owned();
            let proof = self.generate_proof(height, round, proposal.clone(), vote)?;
            if let Some(prev_hash) = &self.prev_hash {
                let res = Commit {
                    height,
                    prev_hash: prev_hash.to_owned(),
                    result: proposal,
                    address: commit.address,
                    proof: proof.clone(),
                };
                self.proof = Some(proof);
                return Ok(res);
            } else {
                return Err(ConsensusError::MissingPrevHash);
            }
        } else {
            Err(ConsensusError::LoseBlock)
        }
    }

    fn ask_for_proposal(&mut self, height: u64) {
        let func = self.function.clone();
        let sender = self.async_send.clone();

        crossbeam_thread::scope(|s| {
            s.spawn(|_| {
                if let Ok(proposal) = func.get_content(height) {
                    sender
                        .send(AsyncMsg::Feed(Feed {
                            content: proposal,
                            height,
                        }))
                        .unwrap();
                }
            });
        })
        .unwrap();
    }

    fn verify_proposal(&mut self, proposal: F) {
        let func = self.function.clone();
        let height = self.height;
        let sender = self.async_send.clone();

        crossbeam_thread::scope(|s| {
            s.spawn(|_| {
                let is_pass = func.check_proposal(proposal.clone(), height).is_ok();
                info!("Receive verify result {:?} at height {:?}", is_pass, height);

                let hash = if cfg!(feature = "turbo_hash") {
                    self.function.hash(&turbo_hash(proposal.rlp_bytes()))
                } else {
                    self.function.hash(&proposal.rlp_bytes())
                };

                sender
                    .send(AsyncMsg::VerifyResp(VerifyResp {
                        is_pass,
                        proposal: hash,
                    }))
                    .unwrap();
            });
        })
        .unwrap();
    }

    fn generate_proof(
        &mut self,
        height: u64,
        round: u64,
        block_hash: F,
        vote: Vec<BftVote>,
    ) -> Result<Proof<F>> {
        let mut tmp = HashMap::new();
        if let Some(vote_set) = self.votes.get_vote_set(height, round, VoteType::Precommit) {
            for v in vote.into_iter() {
                if let Some(signed_vote) = vote_set.vote_pair.get(&v) {
                    tmp.entry(v.voter)
                        .or_insert_with(|| signed_vote.signature.clone());
                } else {
                    return Err(ConsensusError::LoseSignedVote);
                }
            }
        } else {
            return Err(ConsensusError::NoVoteset);
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
        msg: SignedProposal<F>,
        need_wal: bool,
    ) -> Result<(BftProposal, Option<VerifyResp>)> {
        // check signature
        let sig = msg.signature.clone();
        let proposal = msg.proposal.clone();
        let hash = self.function.hash(&proposal.rlp_bytes());
        let address = self
            .function
            .verify_signature(&sig, &hash)
            .map_err(|_| ConsensusError::SupportErr)?;

        let height = proposal.height;
        let round = proposal.round;

        if proposal.proposer != address {
            return Err(ConsensusError::SignatureErr);
        }

        // check height
        if height < self.height - 1 {
            warn!(
                "The height of signed_proposal is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::ObsoleteMsg);
        }
        let content = proposal.content.clone();

        let hash = if cfg!(feature = "turbo_hash") {
            self.function.hash(&turbo_hash(content.rlp_bytes()))
        } else {
            self.function.hash(&content.rlp_bytes())
        };

        self.block_cache
            .entry(hash.clone())
            .or_insert_with(|| content.clone());

        if height >= self.height {
            if height - self.height < CACHE_NUMBER as u64 && need_wal {
                if let Ok(res) = to_string(&msg) {
                    if self
                        .wal_log
                        .save(height, LOG_TYPE_SIGNED_PROPOSAL, res)
                        .is_err()
                    {
                        return Err(ConsensusError::SaveWalErr);
                    }
                } else {
                    return Err(ConsensusError::SerJsonErr);
                }
            }
            if height > self.height {
                self.proposal_cache
                    .entry(height)
                    .or_insert_with(Vec::new)
                    .push(msg);
                info!(
                    "The height of signed_proposal is {} which is higher than self.height {}",
                    height, self.height
                );
                return Err(ConsensusError::FutureMsg);
            }
        }

        self.proposals.add(height, round, &msg);
        let bft_proposal = proposal.to_bft_proposal(hash.clone());

        self.check_proposer(height, round, &bft_proposal.proposer)?;
        self.check_lock_votes(&msg, content.rlp_bytes())?;

        if height == self.height - 1 {
            return Ok((
                bft_proposal,
                Some(VerifyResp {
                    is_pass: true,
                    proposal: hash.clone(),
                }),
            ));
        }

        let verify_res = self.verified_block.get(&hash);
        let verify_resp = if verify_res.is_some() {
            Some(VerifyResp {
                is_pass: *verify_res.unwrap(),
                proposal: hash.clone(),
            })
        } else {
            self.verify_proposal(msg.proposal.content);
            None
        };

        self.check_proof(height, &proposal.proof)?;
        Ok((bft_proposal, verify_resp))
    }

    fn handle_signed_vote(&mut self, msg: SignedVote, need_wal: bool) -> Result<BftVote> {
        let sig = msg.signature.clone();
        let vote = msg.vote.clone();

        // check signature
        let hash = self.function.hash(&msg.rlp_bytes());
        let address = self
            .function
            .verify_signature(&sig, &hash)
            .map_err(|_| ConsensusError::SupportErr)?;

        let height = vote.height;
        let round = vote.round;
        let sender = vote.voter.clone();
        if height < self.height - 1 {
            warn!(
                "The height of raw_bytes is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::ObsoleteMsg);
        }
        let address = address;
        if sender != address {
            error!("The address recovers from the signature is {:?} which is mismatching with the sender {:?}!", &address, &sender);
            return Err(ConsensusError::SignatureErr);
        }

        let bft_vote = vote.to_bft_vote();

        if height >= self.height {
            if height - self.height < CACHE_NUMBER as u64 && need_wal {
                if let Ok(res) = to_string(&msg) {
                    if self.wal_log.save(height, LOG_TYPE_RAW_BYTES, res).is_err() {
                        return Err(ConsensusError::SaveWalErr);
                    }
                } else {
                    return Err(ConsensusError::SerJsonErr);
                }
            }
            if height > self.height {
                self.vote_cache
                    .entry(height)
                    .or_insert_with(Vec::new)
                    .push(msg);
                info!(
                    "The height of raw_bytes is {} which is higher than self.height {}!",
                    height, self.height
                );
                return Err(ConsensusError::FutureMsg);
            }
        }
        self.votes
            .add(height, round, vote.vote_type, &bft_vote, &msg);
        self.check_vote_sender(height, &sender)?;
        Ok(bft_vote)
    }

    fn handle_rich_status(&mut self, msg: Status, need_wal: bool) -> Result<BftStatus> {
        let rich_status = msg;
        let height = rich_status.height;
        if height < self.height {
            warn!(
                "The height of rich_status is {} which is obsolete compared to self.height {}!",
                height, self.height
            );
            return Err(ConsensusError::ObsoleteMsg);
        }

        if need_wal {
            if let Ok(msg) = to_string(&rich_status) {
                if self
                    .wal_log
                    .save(height, LOG_TYPE_RICH_STATUS, msg)
                    .is_err()
                {
                    return Err(ConsensusError::SaveWalErr);
                }
            } else {
                return Err(ConsensusError::SerJsonErr);
            }
        }

        let prev_hash = rich_status.prev_hash.clone();
        self.prev_hash = Some(prev_hash);
        self.authority
            .update_authority(self.height, rich_status.authority_list.clone());

        self.goto_new_height(height)?;
        Ok(rich_status.to_bft_status())
    }

    fn check_proposer(&self, height: u64, round: u64, address: &[u8]) -> Result<()> {
        let authorities = if height == self.authority.authority_h_old {
            &self.authority.authorities_old
        } else {
            &self.authority.authorities
        };

        if (*authorities).is_empty() {
            error!("The size of authority manage is empty!");
            return Err(ConsensusError::NoAuthorityList);
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
            Err(ConsensusError::InvalidProposer)
        }
    }

    fn check_lock_votes(
        &mut self,
        signed_proposal: &SignedProposal<F>,
        proposal_hash: Hash,
    ) -> Result<()> {
        let proposal = &signed_proposal.proposal;
        let height = proposal.height;

        if proposal.lock_round.is_none() {
            return Ok(());
        }

        let mut set = HashSet::new();
        let lock_round = proposal.lock_round.unwrap();
        let votes = proposal.lock_votes.clone();
        for vote in votes.into_iter() {
            let sender = self.check_signed_vote(height, lock_round, proposal_hash.clone(), vote)?;
            if !set.insert(sender) {
                return Err(ConsensusError::BlockVerifyDiff);
            }
        }

        let authority_n = if height == self.authority.authority_h_old {
            info!("Consensus sets the authority manage with old authorities!");
            into_addr_set(self.authority.authorities_old.clone())
        } else {
            into_addr_set(self.authority.authorities.clone())
        };

        if set.len() * 3 > authority_n.len() * 2 {
            for sender in set.into_iter() {
                if !authority_n.contains(&sender) {
                    return Err(ConsensusError::IllegalProposalLock);
                }
            }
            return Ok(());
        }
        Err(ConsensusError::IllegalProposalLock)
    }

    fn check_signed_vote(
        &mut self,
        height: u64,
        round: u64,
        proposal_hash: Hash,
        signed_vote: SignedVote,
    ) -> Result<Address> {
        let vote = signed_vote.vote.clone();
        if height < self.height - 1 {
            error!(
                "The vote's height {} is less than self.height {} - 1, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::ObsoleteMsg);
        }

        let authorities = if height == self.authority.authority_h_old {
            info!("Consensus sets the authority manage with old authorities!");
            &self.authority.authorities_old
        } else {
            &self.authority.authorities
        };

        let hash = vote.proposal.clone();
        if hash != proposal_hash {
            return Err(ConsensusError::BlockVerifyDiff);
        }

        let sender = vote.voter.clone();
        if !authorities.contains(&Node {
            address: sender.clone(),
            proposal_weight: 1,
            vote_weight: 1,
        }) {
            return Err(ConsensusError::InvalidVoter);
        }

        let sig = signed_vote.signature.clone();
        let hash = self.function.hash(&vote.rlp_bytes());

        let address = self.function.verify_signature(&sig, &hash);
        if address.is_err() {
            return Err(ConsensusError::SupportErr);
        }
        if address.unwrap() != sender {
            error!(
                "The address recovers from the signature is mismatching with the sender {:?}!",
                &sender
            );
            return Err(ConsensusError::SignatureErr);
        }

        let bft_vote = vote.to_bft_vote();
        self.votes
            .add(height, round, VoteType::Prevote, &bft_vote, &signed_vote);
        Ok(sender)
    }

    fn check_proof(&mut self, height: u64, proof: &Proof<F>) -> Result<()> {
        if height != self.height {
            error!(
                "The height {} is less than self.height {}, which should not happen!",
                height, self.height
            );
            return Err(ConsensusError::ProofErr);
        }

        if self.authority.authority_h_old == height - 1 {
            if !self.verify_proof(&proof, height - 1, &self.authority.authorities_old) {
                error!("The proof of the block verified failed with old authorities!");
                return Err(ConsensusError::ProofErr);
            }
        } else if !self.verify_proof(&proof, height - 1, &self.authority.authorities) {
            error!("The proof of the block verified failed with newest authorities!");
            return Err(ConsensusError::ProofErr);
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

    fn check_vote_sender(&self, height: u64, sender: &[u8]) -> Result<()> {
        let authorities = if height == self.authority.authority_h_old {
            info!("Consensus sets the authority manage with old authorities!");
            into_addr_set(self.authority.authorities_old.clone())
        } else {
            into_addr_set(self.authority.authorities.clone())
        };

        if !authorities.contains(&sender.to_vec()) {
            error!("The raw_bytes have invalid voter {:?}!", &sender);
            return Err(ConsensusError::InvalidVoter);
        }
        Ok(())
    }

    fn goto_new_height(&mut self, height: u64) -> Result<()> {
        self.verified_block.clear();
        self.block_cache.clear();
        self.block = None;
        self.height = height + 1;
        if self.wal_log.set_height(self.height).is_err() {
            error!("Wal log set height {} failed!", self.height);
            return Err(ConsensusError::SaveWalErr);
        };
        Ok(())
    }

    fn verify_proof(&self, proof: &Proof<F>, h: u64, authority: &[Node]) -> bool {
        if h == 0 {
            return true;
        }

        let authority = into_addr_set(authority.to_vec());
        if h != proof.height || 2 * authority.len() >= 3 * proof.precommit_votes.len() {
            return false;
        }

        for (sender, sig) in proof.precommit_votes.clone().into_iter() {
            if authority.contains(&sender) {
                let proposal = if cfg!(feature = "turbo_hash") {
                    self.function
                        .hash(&turbo_hash(proof.block_hash.rlp_bytes()))
                } else {
                    self.function.hash(&proof.block_hash.rlp_bytes())
                };

                let msg = Vote {
                    vote_type: VoteType::Precommit,
                    height: proof.height,
                    round: proof.round,
                    proposal,
                    voter: sender.clone(),
                };
                let hash = self.function.hash(&msg.rlp_bytes());

                if let Ok(address) = self.function.verify_signature(&sig, &hash) {
                    if address != sender {
                        return false;
                    }
                }
            } else {
                return false;
            }
        }
        true
    }

    #[warn(dead_code)]
    fn load_wal_log(&mut self) {
        info!("Consensus starts to load wal log!");
        let vec_buf = self.wal_log.load();
        for (msg_type, msg) in vec_buf {
            match msg_type {
                LOG_TYPE_SIGNED_PROPOSAL => {
                    info!("Consensus loads signed_proposal");
                    let msg: SignedProposal<F> =
                        from_slice(&msg).expect("Try from message failed!");
                    if let Ok((proposal, verify_resp)) = self.handle_signed_proposal(msg, false) {
                        info!("Consensus hands over bft_proposal to bft-rs");
                        self.bft
                            .send_bft_msg(CoreInput::Proposal(proposal))
                            .expect("Consensus hands over bft_proposal failed!");
                        if let Some(verify_resp) = verify_resp {
                            info!("Consensus hands over verify_resp to bft-rs");
                            self.bft
                                .send_bft_msg(CoreInput::VerifyResp(verify_resp.to_bft_resp()))
                                .expect("Consensus hands over verify_resp failed!");
                        }
                    };
                }
                LOG_TYPE_RAW_BYTES => {
                    info!("Consensus loads raw_bytes message");
                    let msg: SignedVote = from_slice(&msg).expect("Try from message failed!");
                    if let Ok(vote) = self.handle_signed_vote(msg, false) {
                        info!("Consensus hands over bft_vote to bft-rs");
                        self.bft
                            .send_bft_msg(CoreInput::Vote(vote))
                            .expect("Consensus hands over bft_vote failed!");
                    };
                }
                LOG_TYPE_RICH_STATUS => {
                    info!("Consensus loads rich_status message");
                    let msg: Status = from_slice(&msg).expect("Try from message failed!");
                    if let Ok(status) = self.handle_rich_status(msg, false) {
                        info!("Consensus hands over bft_status to bft-rs");
                        self.bft
                            .send_bft_msg(CoreInput::Status(status))
                            .expect("Consensus hands over bft_status failed!");
                    };
                }
                LOG_TYPE_BLOCK_TXS => {
                    info!("Consensus loads block_txs message");
                    let msg: Feed<F> = from_slice(&msg).expect("Try from message failed!");
                    let hash = if cfg!(feature = "turbo_hash") {
                        self.function.hash(&turbo_hash(msg.content.rlp_bytes()))
                    } else {
                        self.function.hash(&msg.content.rlp_bytes())
                    };

                    self.bft
                        .send_bft_msg(CoreInput::Feed(BftFeed {
                            height: msg.height,
                            proposal: hash.clone(),
                        }))
                        .expect("Consensus hands over bft_status failed!");
                    self.block_cache
                        .entry(hash)
                        .or_insert_with(|| msg.content.clone());
                }
                LOG_TYPE_VERIFY_BLOCK_PESP => {
                    info!("Consensus loads verify_block_resp message");
                    let msg: VerifyResp = from_slice(&msg).expect("Try from message failed!");
                    let hash = if cfg!(feature = "turbo_hash") {
                        self.function.hash(&turbo_hash(msg.proposal.rlp_bytes()))
                    } else {
                        self.function.hash(&msg.proposal.rlp_bytes())
                    };

                    self.bft
                        .send_bft_msg(CoreInput::VerifyResp(BftVerifyResp {
                            is_pass: msg.is_pass,
                            proposal: hash.clone(),
                        }))
                        .expect("Consensus hands over bft_status failed!");
                    self.verified_block.entry(hash).or_insert(msg.is_pass);
                }
                LOG_TYPE_PROPOSAL => {
                    info!("Consensus loads bft_proposal message");
                    let proposal: BftProposal =
                        from_slice(&msg).expect("Deserialize message failed!");
                    if let Ok(signed_proposal) = self.handle_proposal(proposal.clone(), false) {
                        info!(
                            "Consensus sends signed_proposal to rabbit_mq!\n{:?}",
                            proposal
                        );
                        self.function
                            .transmit(ConsensusOutput::SignedProposal(signed_proposal))
                            .expect("Consensus sends signed_proposal failed!");;
                    };
                }
                LOG_TYPE_VOTE => {
                    info!("Consensus loads bft_vote message");
                    let vote: BftVote = from_slice(&msg).expect("Deserialize message failed!");
                    if let Ok(raw_bytes) = self.handle_vote(vote.clone(), false) {
                        info!("Consensus sends raw_bytes to rabbit_mq!\n{:?}", vote);
                        self.function
                            .transmit(ConsensusOutput::SignedVote(raw_bytes))
                            .expect("Consensus sends raw_bytes failed!");
                    };
                }
                LOG_TYPE_COMMIT => {
                    info!("Consensus loads bft_commit message!");
                    let commit: BftCommit = from_slice(&msg).expect("Deserialize message failed!");
                    if let Ok(block_with_proof) = self.handle_commit(commit.clone(), true) {
                        info!(
                            "Consensus sends block_with_proof to rabbit_mq!\n{:?}",
                            commit
                        );
                        self.function
                            .commit(block_with_proof)
                            .expect("Consensus sends block_with_proof failed!");
                    };
                }
                _ => {}
            }
        }
        info!("Consensus successfully processes the whole wal log!");
    }
}
