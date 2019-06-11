use crate::{consensus::Result as CResult, error::ConsensusError, types::*};
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

pub(crate) fn combine(first: &[u8], second: &[u8]) -> Vec<u8> {
    let first_len = first.len() as u64;
    let len_mark = first_len.to_be_bytes();
    let mut encode = Vec::with_capacity(8 + first.len() + second.len());
    encode.extend_from_slice(&len_mark);
    encode.extend_from_slice(first);
    encode.extend_from_slice(second);
    encode
}

pub(crate) fn encode_block(height: u64, block: &[u8], block_hash: &[u8]) -> Vec<u8> {
    let height_mark = height.to_be_bytes();
    let mut encode = Vec::with_capacity(8 + block.len());
    encode.extend_from_slice(&height_mark);
    let combine = combine(&block_hash, &block);
    encode.extend_from_slice(&combine);
    encode
}

pub(crate) fn extract(encode: &[u8]) -> CResult<(&[u8], &[u8])> {
    let encode_len = encode.len();
    if encode_len < 8 {
        return Err(ConsensusError::DecodeErr("Bytes is too short".to_string()));
    }
    let mut len: [u8; 8] = [0; 8];
    len.copy_from_slice(&encode[0..8]);
    let first_len = u64::from_be_bytes(len) as usize;
    if encode_len < first_len + 8 {
        return Err(ConsensusError::DecodeErr("Bytes is too short".to_string()));
    }
    let (combine, two) = encode.split_at(first_len + 8);
    let (_, one) = combine.split_at(8);
    Ok((one, two))
}

pub(crate) fn decode_block(encode: &[u8]) -> CResult<(u64, Vec<u8>, Hash)> {
    let (h, combine) = encode.split_at(8);
    let (block_hash, block) = extract(combine)?;
    let mut height_mark: [u8; 8] = [0; 8];
    height_mark.copy_from_slice(h);
    let height = u64::from_be_bytes(height_mark);
    Ok((height, block.into(), block_hash.into()))
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
