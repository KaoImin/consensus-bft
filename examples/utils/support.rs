use consensus_bft::{
    types::{Address, Commit, ConsensusOutput, Hash, Node, Signature, Status},
    ConsensusSupport, Content,
};
use crossbeam_channel::Receiver;

#[derive(Clone, Debug)]
pub enum Error {
    SupportError,
}

#[derive(Clone, Debug)]
pub(crate) struct Support<F: Content + Sync> {
    address: Vec<u8>,
    recv: Receiver<F>,
}

impl<F> Support<F>
where
    F: Content + Sync,
{
    pub(crate) fn new(address: Vec<u8>, recv: Receiver<F>) -> Self {
        Support { address, recv }
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

    fn check_proposal(
        &self,
        _block_hash: &[u8],
        _block: &F,
        _signed_proposal_hash: &[u8],
        _height: u64,
        _is_lock: bool,
        _is_by_self: bool,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn commit(&self, commit: Commit<F>) -> Result<Status, Self::Error> {
        let status = Status {
            height: commit.height,
            interval: None,
            authority_list: vec![Node::new(vec![1, 2, 3])],
        };
        Ok(status)
    }

    fn sign(&self, _hash: &[u8]) -> Result<Signature, Self::Error> {
        Ok(self.address.clone())
    }

    fn verify_signature(&self, _signature: &[u8], _hash: &[u8]) -> Result<Address, Self::Error> {
        Ok(self.address.clone())
    }

    fn hash(&self, msg: &[u8]) -> Hash {
        msg.to_vec()
    }

    fn get_content(&self, _height: u64) -> Result<F, Self::Error> {
        self.recv.recv().map_err(|_| Error::SupportError)
    }
}
