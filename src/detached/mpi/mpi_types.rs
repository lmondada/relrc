use mpi::{datatype::DatatypeRef, traits::Equivalence};

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
