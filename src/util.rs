use crate::types::*;
use bft_core::{types::CoreOutput, FromCore};
use crossbeam_channel::Sender;
use rlp::Encodable;

/// An independent function to check proof.
pub fn check_proof(
    proof: &Proof,
    height: u64,
    authority: &[Node],
    crypt_hash: impl Fn(&[u8]) -> Vec<u8>,
    verify_signature: impl Fn(&[u8], &[u8]) -> Option<Address>,
) -> bool {
    if height == 0 {
        return true;
    }

    if height != proof.height + 1 {
        return false;
    }

    let vote_addresses: Vec<Address> = proof
        .precommit_votes
        .iter()
        .map(|(sender, _)| sender.clone())
        .collect();

    if get_votes_weight(authority, &vote_addresses) * 3 <= get_total_weight(authority) * 2 {
        return false;
    }
    let addr_set = into_addr_set(authority.to_owned());

    for (sender, sig) in proof.to_owned().precommit_votes.into_iter() {
        if addr_set.contains(&sender) {
            let msg = Vote {
                vote_type: VoteType::Precommit,
                height: proof.height,
                round: proof.round,
                proposal: proof.block_hash.clone(),
                voter: sender.clone(),
            };
            let hash = crypt_hash(&msg.rlp_bytes());

            if Some(sender) != verify_signature(&sig, &hash) {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

///
#[inline]
pub fn get_total_weight(authorities: &[Node]) -> u64 {
    let weight: Vec<u64> = authorities
        .iter()
        .map(|node| u64::from(node.vote_weight))
        .collect();
    weight.iter().sum()
}

///
#[inline]
pub fn get_votes_weight(authorities: &[Node], vote_addresses: &[Address]) -> u64 {
    let votes_weight: Vec<u64> = authorities
        .iter()
        .filter(|node| vote_addresses.contains(&node.address))
        .map(|node| u64::from(node.vote_weight))
        .collect();
    votes_weight.iter().sum()
}

pub(crate) fn into_addr_set(node_set: Vec<Node>) -> Vec<Address> {
    let mut set = Vec::new();
    for node in node_set.into_iter() {
        set.push(node.address);
    }
    set
}

#[derive(Debug)]
pub(crate) enum Error {
    SendMsgErr,
}

#[derive(Debug, Clone)]
pub(crate) struct SendMsg(Sender<CoreOutput>);

impl FromCore for SendMsg {
    type error = Error;

    fn send_msg(&self, msg: CoreOutput) -> Result<(), Error> {
        self.0.send(msg).map_err(|_| Error::SendMsgErr)?;
        Ok(())
    }
}

impl SendMsg {
    pub(crate) fn new(s: Sender<CoreOutput>) -> Self {
        SendMsg(s)
    }
}
