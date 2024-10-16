use mpi::{datatype::DatatypeRef, traits::Equivalence};

use crate::hash_id::RelRcHash;

pub(super) enum MPIMessage<N, E> {
    RelRc(RelRcHash),
    NodeWeight(N),
    IncomingEdge(Vec<RelRcHash>),
    EdgeWeight(E),
    // both below correspond to tag Ack (distinguished by a non-zero value)
    RequestRelRc(RelRcHash),
    Done,
}

/// All message types used in the MPI communication.
///
/// We use MPI Tags to "strongly type" communication.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MPIMessageTag {
    // tags for messags sent from sender to receiver
    RelRc = 0,
    NodeWeight = 1,
    IncomingEdge = 2,
    EdgeWeight = 3,

    // tags for messages sent from receiver to sender
    /// Acknowledge the receipt of a message. If the value of Ack is non-zero,
    /// then further [`RelRc`] are requested.
    Ack = 100,
}

impl<N, E> MPIMessage<N, E> {
    pub(super) fn tag(&self) -> MPIMessageTag {
        match self {
            MPIMessage::RelRc { .. } => MPIMessageTag::RelRc,
            MPIMessage::NodeWeight { .. } => MPIMessageTag::NodeWeight,
            MPIMessage::IncomingEdge { .. } => MPIMessageTag::IncomingEdge,
            MPIMessage::EdgeWeight { .. } => MPIMessageTag::EdgeWeight,
            MPIMessage::RequestRelRc { .. } | MPIMessage::Done => MPIMessageTag::Ack,
        }
    }
}

impl TryFrom<i32> for MPIMessageTag {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MPIMessageTag::RelRc),
            1 => Ok(MPIMessageTag::NodeWeight),
            2 => Ok(MPIMessageTag::IncomingEdge),
            3 => Ok(MPIMessageTag::EdgeWeight),
            100 => Ok(MPIMessageTag::Ack),
            _ => Err(()),
        }
    }
}

#[repr(transparent)]
pub(super) struct MPIRelRc {
    pub(super) hash: usize,
    // value: N, may be a variable length vec, separate message type
    // incoming is a variable length vec, separate message type
}

unsafe impl Equivalence for MPIRelRc {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        usize::equivalent_datatype()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub(super) struct MPIIncomingEdge {
    pub(super) source_hash: usize,
    // value: E, may be a variable length vec, separate message type
}

unsafe impl Equivalence for MPIIncomingEdge {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        usize::equivalent_datatype()
    }
}

#[repr(transparent)]
pub(super) struct MPIAck {
    pub(super) hash: usize,
}

unsafe impl Equivalence for MPIAck {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        usize::equivalent_datatype()
    }
}

impl<N, E> From<MPIRelRc> for MPIMessage<N, E> {
    fn from(val: MPIRelRc) -> Self {
        MPIMessage::RelRc(val.hash.into())
    }
}

impl<N, E> From<Vec<MPIIncomingEdge>> for MPIMessage<N, E> {
    fn from(val: Vec<MPIIncomingEdge>) -> Self {
        MPIMessage::IncomingEdge(val.into_iter().map(|e| e.source_hash.into()).collect())
    }
}

impl<N, E> From<MPIAck> for MPIMessage<N, E> {
    fn from(val: MPIAck) -> Self {
        if val.hash == 0 {
            MPIMessage::Done
        } else {
            MPIMessage::RequestRelRc(val.hash.into())
        }
    }
}

impl From<MPIRelRc> for RelRcHash {
    fn from(msg: MPIRelRc) -> Self {
        msg.hash.into()
    }
}
