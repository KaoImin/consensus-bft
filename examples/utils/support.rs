use consensus_bft::{
    types::{Address, Commit, ConsensusOutput},
    ConsensusSupport,
};
use crossbeam_channel::{unbounded, Receiver, Sender};
use rand::random;
use rlp::{Decodable, DecoderError, Encodable, Rlp, RlpStream};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Clone, Debug)]
pub enum Error {
    SupportError,
}

#[derive(Clone, Debug)]
pub struct Support<Content> {
    commit: Sender<Commit<Content>>,
    address: Vec<u8>,
}

pub struct Content(Vec<u8>);

impl Content {
    pub(crate) fn new(size: usize) -> Self {
        let mb = (0..1024 * 1024 * size)
            .map(|_| random::<u8>())
            .collect::<Vec<_>>();
        Content(mb)
    }
}

impl Encodable for Content {
    fn rlp_append(&self, s: &mut RlpStream) {
        s.append_list(&self.0);
    }
}

impl Decodable for Content {
    fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
        let res = rlp
            .as_list::<u8>()
            .map_err(|_| DecoderError::RlpExpectedToBeList)?;
        Ok(Content(res))
    }
}

impl<F> ConsensusSupport<F> for Support<Content>
where
    F: Encodable + Decodable + Clone + Send + 'static + Serialize + DeserializeOwned,
{
    type Error = Error;

    fn transmit(&self, msg: ConsensusOutput<Content>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn check_block(&self, block: Content, height: u64) -> Result<(), Self::Error> {
        Ok(())
    }

    fn commit(&self, commit: Commit<Content>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn sign(&self, hash: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(self.address.clone())
    }

    fn check_signature(&self, signature: &[u8], hash: &[u8]) -> Result<Address, Self::Error> {
        Ok(self.address.clone())
    }

    fn hash(&self, msg: &[u8]) -> Vec<u8> {
        msg.to_vec()
    }

    fn get_block(&self, height: u64) -> Result<Content, Self::Error> {
        Ok(Content::new(1))
    }
}
