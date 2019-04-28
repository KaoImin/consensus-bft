use consensus_bft::{
    types::{Address, Commit, ConsensusOutput},
    ConsensusSupport, Content,
};
use crossbeam_channel::{Receiver, Sender};

#[derive(Clone, Debug)]
pub(crate) enum Error {
    SupportError,
}

#[derive(Clone, Debug)]
pub(crate) struct Support<F: Content + Sync> {
    address: Vec<u8>,
    send: Sender<u64>,
    recv: Receiver<F>,
}

impl<F> Support<F>
where
    F: Content + Sync,
{
    pub(crate) fn new(address: Vec<u8>, send: Sender<u64>, recv: Receiver<F>) -> Self {
        Support {
            address,
            send,
            recv,
        }
    }
}

impl<F> ConsensusSupport<F> for Support<F>
where
    F: Content + Sync,
{
    type Error = Error;

    fn transmit(&self, _msg: ConsensusOutput<F>) -> Result<(), Self::Error> {
        Ok(())
    }

    fn check_block(&self, _block: F, _height: u64) -> Result<(), Self::Error> {
        Ok(())
    }

    fn commit(&self, commit: Commit<F>) -> Result<(), Self::Error> {
        self.send.send(commit.height).unwrap();
        Ok(())
    }

    fn sign(&self, _hash: &[u8]) -> Result<Vec<u8>, Self::Error> {
        Ok(self.address.clone())
    }

    fn check_signature(&self, _signature: &[u8], _hash: &[u8]) -> Result<Address, Self::Error> {
        Ok(self.address.clone())
    }

    fn hash(&self, msg: &[u8]) -> Vec<u8> {
        msg.to_vec()
    }

    fn get_block(&self, _height: u64) -> Result<F, Self::Error> {
        self.recv.recv().map_err(|_| Error::SupportError)
    }
}
