mod utils;

use consensus_bft::{
    consensus::ConsensusExecutor,
    types::{ConsensusInput, Node, Status},
    Content,
};
use crossbeam_channel::bounded;
use env_logger::Builder;
use rand::random;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use serde::{Deserialize, Serialize};
use std::thread;
use utils::support::{Error, Support};

#[derive(Clone, Debug, Hash, Serialize, Deserialize)]
pub struct BftContent(Vec<u8>);

impl Content for BftContent {
    type Error = Error;

    fn encode(self) -> Result<Vec<u8>, Self::Error> {
        Ok(self.0.clone())
    }

    fn decode(_bytes: &[u8]) -> Result<Self, Self::Error> {
        Ok(BftContent(vec![]))
    }

    fn hash(&self) -> Vec<u8> {
        self.0.clone()
    }
}

impl BftContent {
    pub(crate) fn new(size: usize) -> Self {
        let mb = (0..10 * size).map(|_| random::<u8>()).collect::<Vec<_>>();
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
    let mut builder = Builder::from_default_env();
    builder.filter(None, log::LevelFilter::Debug).init();

    let (s_proposal, r_proposal) = bounded::<BftContent>(1);
    let (s_signal, r_signal) = bounded::<u64>(1);
    let support = Support::new(vec![1, 2, 3], s_signal, r_proposal);
    let executor = ConsensusExecutor::new(support, vec![1, 2, 3], "examples/wal/log");

    thread::spawn(move || {
        executor
            .send(ConsensusInput::Status(Status {
                height: 0,
                interval: None,
                authority_list: vec![Node::new(vec![1, 2, 3])],
            }))
            .unwrap();
        println!("Send Genesis");

        loop {
            let msg = BftContent::new(1);
            s_proposal.send(msg.clone()).unwrap();
            println!("Send proposal");

            if let Ok(height) = r_signal.recv() {
                println!("Send status at height {:?}", height);
                executor
                    .send(ConsensusInput::Status(Status {
                        height,
                        interval: None,
                        authority_list: vec![Node::new(vec![1, 2, 3])],
                    }))
                    .unwrap();
            }
        }
    })
    .join()
    .unwrap();
}
