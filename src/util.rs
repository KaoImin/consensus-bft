use crate::types::*;
use rlp::{Decodable, Encodable};
use serde::{de::DeserializeOwned, Serialize};

///
pub fn check_proof<
    F: Encodable + Decodable + Encodable + Clone + Send + 'static + Serialize + DeserializeOwned,
>(
    proof: Proof<F>,
    height: u64,
    authority: Vec<Node>,
    crypt_hash: fn(msg: &[u8]) -> Vec<u8>,
    check_signature: fn(signature: &[u8], hash: &[u8]) -> Option<Address>,
) -> bool {
    if height == 0 {
        return true;
    }

    let authority = into_addr_set(authority);
    if height != proof.height || 2 * authority.len() >= 3 * proof.precommit_votes.len() {
        return false;
    }

    for (sender, sig) in proof.precommit_votes.into_iter() {
        if authority.contains(&sender) {
            let msg = Vote {
                vote_type: VoteType::Precommit,
                height: proof.height,
                round: proof.round,
                proposal: crypt_hash(&proof.block_hash.rlp_bytes()),
                voter: sender.clone(),
            };
            let hash = crypt_hash(&msg.rlp_bytes());

            if Some(sender) != check_signature(&sig, &hash) {
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
