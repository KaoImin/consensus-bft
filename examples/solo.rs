mod utils;

use consensus_bft::{
    consensus::ConsensusExecutor,
    types::{ConsensusInput, Node, Status},
    Content,
};
use crossbeam_channel::bounded;
use rand::random;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use std::thread;
use utils::support::Support;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BftContent(Vec<u8>);

impl Content for BftContent {}

impl BftContent {
    pub(crate) fn new(size: usize) -> Self {
        let mb = (0..1024 * 1024 * size)
            .map(|_| random::<u8>())
            .collect::<Vec<_>>();
        BftContent(mb)
    }
}

impl Encodable for BftContent {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_list(&self.0);
    }
}

impl Decodable for BftContent {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let res = rlp
            .as_list::<u8>()
            .map_err(|_| DecoderError::RlpExpectedToBeList)?;
        Ok(BftContent(res))
    }
}

fn main() {
    let (s_proposal, r_proposal) = bounded::<BftContent>(1);
    let (s_signal, r_signal) = bounded::<u64>(1);
    let support = Support::new(vec![1, 2, 3], s_signal, r_proposal);
    let executor = ConsensusExecutor::new(support, vec![1, 2, 3], "example/wal");

    thread::spawn(move || {
        executor
            .send(ConsensusInput::Status(Status {
                height: 1,
                interval: None,
                authority_list: vec![Node::new(vec![1, 2, 3])],
                prev_hash: vec![0, 0, 0],
            }))
            .unwrap();

        loop {
            s_proposal.send(BftContent::new(1)).unwrap();
            println!("Send proposal");

            if let Ok(height) = r_signal.recv() {
                println!("Send status at height {:?}", height);
                executor
                    .send(ConsensusInput::Status(Status {
                        height,
                        interval: None,
                        authority_list: vec![Node::new(vec![1, 2, 3])],
                        prev_hash: (0..160).map(|_| random::<u8>()).collect::<Vec<_>>(),
                    }))
                    .unwrap();
            }
        }
    });
}
