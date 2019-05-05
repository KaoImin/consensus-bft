use crate::types::*;
use bft_core::{types::CoreOutput, FromCore};
use crossbeam_channel::Sender;
use rlp::{Decodable, Encodable};
use serde::{de::DeserializeOwned, Serialize};

/// An independent function to check proof.
pub fn check_proof<
    F: Encodable + Decodable + Encodable + Clone + Send + 'static + Serialize + DeserializeOwned,
>(
    proof: Proof<F>,
    height: u64,
    authority: Vec<Node>,
    crypt_hash: impl Fn(&[u8]) -> Vec<u8>,
    verify_signature: impl Fn(&[u8], &[u8]) -> Option<Address>,
    is_turbo: bool,
) -> bool {
    if height == 0 {
        return true;
    }

    let authority = into_addr_set(authority);
    if height != proof.height || 2 * authority.len() >= 3 * proof.precommit_votes.len() {
        return false;
    }

    let proposal = if is_turbo {
        crypt_hash(&turbo_hash(proof.block_hash.rlp_bytes()))
    } else {
        crypt_hash(&proof.block_hash.rlp_bytes())
    };

    for (sender, sig) in proof.precommit_votes.into_iter() {
        if authority.contains(&sender) {
            let msg = Vote {
                vote_type: VoteType::Precommit,
                height: proof.height,
                round: proof.round,
                proposal: proposal.clone(),
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

pub(crate) fn into_addr_set(node_set: Vec<Node>) -> Vec<Address> {
    let mut set = Vec::new();
    for node in node_set.into_iter() {
        set.push(node.address);
    }
    set
}

pub(crate) fn turbo_hash(msg: Vec<u8>) -> Vec<u8> {
    let mut res = Vec::new();
    let length = (msg.len() as f64 / 100.0).round() as usize;
    for i in 0..100 {
        res.push(msg[i * length]);
    }
    res
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
