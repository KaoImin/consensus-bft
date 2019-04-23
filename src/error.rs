use crate::ConsensusSupport;

///
#[derive(Debug, Clone)]
pub enum ConsensusError {
    ///
    BlockVerifyDiff,
    ///
    SendMsgErr,
    ///
    SupportErr,
    ///
    SerJsonErr,
    ///
    SaveWalErr,
    ///
    BftCoreErr,
    ///
    LoseBlock,
    ///
    LoseSignedVote,
    ///
    NoVoteset,
    ///
    SignatureErr,
    ///
    ObsoleteMsg,
    ///
    FutureMsg,
    ///
    NoAuthorityList,
    ///
    InvalidProposer,
    ///
    InvalidVoter,
    ///
    IllegalProposalLock,
}
