use crate::types::*;
use rlp::Encodable;

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
                proposal: proof.block_hash.clone(),
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

pub(crate) fn into_addr_set(node_set: Vec<Node>) -> Vec<Address> {
    let mut set = Vec::new();
    for node in node_set.into_iter() {
        set.push(node.address);
    }
    set
}
