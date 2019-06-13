use crate::types::{SignedProposal, SignedVote, VoteType};
use bft_core::types::Vote as BftVote;
use lru_cache::LruCache;
use std::collections::HashMap;

pub(crate) const CACHE_NUMBER: usize = 16;

#[derive(Debug, Clone)]
pub(crate) struct VoteCollector {
    pub(crate) votes: LruCache<u64, RoundCollector>,
}

impl VoteCollector {
    pub(crate) fn new() -> Self {
        VoteCollector {
            votes: LruCache::new(CACHE_NUMBER),
        }
    }

    pub(crate) fn add(
        &mut self,
        height: u64,
        round: u64,
        vote_type: VoteType,
        bft_vote: &BftVote,
        signed_vote: &SignedVote,
    ) -> bool {
        if self.votes.contains_key(&height) {
            self.votes
                .get_mut(&height)
                .unwrap()
                .add(round, vote_type, bft_vote, signed_vote)
        } else {
            let mut round_votes = RoundCollector::new();
            round_votes.add(round, vote_type, bft_vote, signed_vote);
            self.votes.insert(height, round_votes);
            true
        }
    }

    pub(crate) fn get_vote_set(
        &mut self,
        height: u64,
        round: u64,
        vote_type: VoteType,
    ) -> Option<VoteSet> {
        self.votes
            .get_mut(&height)
            .and_then(|rc| rc.get_vote_set(round, vote_type))
    }
}

//round -> step collector
#[derive(Debug, Clone)]
pub(crate) struct RoundCollector {
    pub round_votes: LruCache<u64, StepCollector>,
}

impl RoundCollector {
    pub(crate) fn new() -> Self {
        RoundCollector {
            round_votes: LruCache::new(CACHE_NUMBER),
        }
    }

    pub(crate) fn add(
        &mut self,
        round: u64,
        vote_type: VoteType,
        bft_vote: &BftVote,
        signed_vote: &SignedVote,
    ) -> bool {
        if self.round_votes.contains_key(&round) {
            self.round_votes
                .get_mut(&round)
                .unwrap()
                .add(vote_type, bft_vote, &signed_vote)
        } else {
            let mut step_votes = StepCollector::new();
            step_votes.add(vote_type, bft_vote, &signed_vote);
            self.round_votes.insert(round, step_votes);
            true
        }
    }

    pub(crate) fn get_vote_set(&mut self, round: u64, vote_type: VoteType) -> Option<VoteSet> {
        self.round_votes
            .get_mut(&round)
            .and_then(|sc| sc.get_vote_set(vote_type))
    }
}

//step -> voteset
#[derive(Debug, Clone)]
pub(crate) struct StepCollector {
    pub(crate) step_votes: HashMap<VoteType, VoteSet>,
}

impl StepCollector {
    pub(crate) fn new() -> Self {
        StepCollector {
            step_votes: HashMap::new(),
        }
    }

    pub(crate) fn add(
        &mut self,
        vote_type: VoteType,
        bft_vote: &BftVote,
        signed_vote: &SignedVote,
    ) -> bool {
        self.step_votes
            .entry(vote_type)
            .or_insert_with(VoteSet::new)
            .add(bft_vote, signed_vote)
    }

    pub fn get_vote_set(&self, vote_type: VoteType) -> Option<VoteSet> {
        self.step_votes.get(&vote_type).cloned()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct VoteSet {
    pub vote_pair: HashMap<BftVote, SignedVote>,
}

impl VoteSet {
    pub(crate) fn new() -> Self {
        VoteSet {
            vote_pair: HashMap::new(),
        }
    }

    //just add ,not check
    pub(crate) fn add(&mut self, bft_vote: &BftVote, signed_vote: &SignedVote) -> bool {
        let mut added = false;
        self.vote_pair.entry(bft_vote.clone()).or_insert_with(|| {
            added = true;
            signed_vote.to_owned()
        });
        added
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProposalCollector {
    pub(crate) proposals: LruCache<u64, ProposalRoundCollector>,
}

impl ProposalCollector {
    pub(crate) fn new() -> Self {
        ProposalCollector {
            proposals: LruCache::new(CACHE_NUMBER),
        }
    }

    pub(crate) fn add(&mut self, height: u64, round: u64, proposal: &SignedProposal) -> bool {
        if self.proposals.contains_key(&height) {
            self.proposals
                .get_mut(&height)
                .unwrap()
                .add(round, proposal)
        } else {
            let mut round_proposals = ProposalRoundCollector::new();
            round_proposals.add(round, proposal);
            self.proposals.insert(height, round_proposals);
            true
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ProposalRoundCollector {
    pub(crate) round_proposals: LruCache<u64, SignedProposal>,
}

impl ProposalRoundCollector {
    pub(crate) fn new() -> Self {
        ProposalRoundCollector {
            round_proposals: LruCache::new(CACHE_NUMBER),
        }
    }

    pub(crate) fn add(&mut self, round: u64, proposal: &SignedProposal) -> bool {
        if self.round_proposals.contains_key(&round) {
            false
        } else {
            self.round_proposals.insert(round, proposal.clone());
            true
        }
    }

    //     pub(crate) fn get_proposal(&mut self, round: u64) -> Option<SignedProposal> {
    //         self.round_proposals.get_mut(&round).cloned()
    //     }
}
