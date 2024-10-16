use mpi::{datatype::DatatypeRef, traits::Equivalence};

use crate::hash_id::RelRcHash;

pub(super) enum MPIMessage<N, E> {
    RelRc(MPIRelRc),
    NodeWeight(N),
    IncomingEdge(Vec<MPIIncomingEdge>),
    EdgeWeight(E),
    RequestRelRc(MPIRequestRelRc),
    Done,
}

/// All message types used in the MPI communication.
///
/// We use MPI Tags to "strongly type" communication.
#[repr(i32)]
pub(super) enum MPIMessageTag {
    RelRc = 0,
    NodeWeight = 1,
    IncomingEdge = 2,
    EdgeWeight = 3,

    RequestRelRc = 100,

    Done = 200,
}

impl TryFrom<i32> for MPIMessageTag {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MPIMessageTag::RelRc),
            1 => Ok(MPIMessageTag::NodeWeight),
            2 => Ok(MPIMessageTag::IncomingEdge),
            3 => Ok(MPIMessageTag::EdgeWeight),
            100 => Ok(MPIMessageTag::RequestRelRc),
            200 => Ok(MPIMessageTag::Done),
            _ => Err(()),
        }
    }
}

unsafe impl Equivalence for MPIMessageTag {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        u8::equivalent_datatype()
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
pub(super) struct MPIRequestRelRc {
    pub(super) hash: usize,
}

unsafe impl Equivalence for MPIRequestRelRc {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        usize::equivalent_datatype()
    }
}

#[repr(transparent)]
pub(super) struct MPIDone(pub(super) bool); // ignore value

unsafe impl Equivalence for MPIDone {
    type Out = DatatypeRef<'static>;

    fn equivalent_datatype() -> Self::Out {
        bool::equivalent_datatype()
    }
}

impl<N, E> Into<MPIMessage<N, E>> for MPIRelRc {
    fn into(self) -> MPIMessage<N, E> {
        MPIMessage::RelRc(self)
    }
}

impl<N, E> Into<MPIMessage<N, E>> for Vec<MPIIncomingEdge> {
    fn into(self) -> MPIMessage<N, E> {
        MPIMessage::IncomingEdge(self)
    }
}

impl<N, E> Into<MPIMessage<N, E>> for MPIRequestRelRc {
    fn into(self) -> MPIMessage<N, E> {
        MPIMessage::RequestRelRc(self)
    }
}

impl<N, E> Into<MPIMessage<N, E>> for MPIDone {
    fn into(self) -> MPIMessage<N, E> {
        MPIMessage::Done
    }
}

impl From<MPIRelRc> for RelRcHash {
    fn from(msg: MPIRelRc) -> Self {
        msg.hash.into()
    }
}
