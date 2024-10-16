use std::{
    future::{self, Future, Ready},
    marker::PhantomData,
};

use mpi::traits::{Destination, Equivalence, Source};

use super::msg_types::{
    MPIDone, MPIIncomingEdge, MPIMessage, MPIMessageTag, MPIRelRc, MPIRequestRelRc,
};

pub(super) trait MPISendRecv<N, E> {
    type ReceiveOut: Future<Output = MPIMessage<N, E>>;

    fn send(&self, msg: &MPIMessage<N, E>);

    fn receive(&self) -> Self::ReceiveOut;
}

/// Send and receive MPI messages using standard communication.
pub(super) struct MPIStandardSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

/// Send and receive MPI messages using buffered communication.
pub(super) struct MPIBufferedSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

/// Send and receive MPI messages using asynchronous communication.
pub(super) struct MPIAsyncSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

macro_rules! generate_send_match {
    ($self:expr, $msg:expr, $send_fn:ident) => {
        match $msg {
            MPIMessage::RelRc(msg) => $self.0.$send_fn(msg, MPIMessageTag::RelRc as i32),
            MPIMessage::NodeWeight(msg) => $self.0.$send_fn(msg, MPIMessageTag::NodeWeight as i32),
            MPIMessage::IncomingEdge(msg) => {
                $self.0.$send_fn(msg, MPIMessageTag::IncomingEdge as i32)
            }
            MPIMessage::EdgeWeight(msg) => $self.0.$send_fn(msg, MPIMessageTag::EdgeWeight as i32),
            MPIMessage::RequestRelRc(msg) => {
                $self.0.$send_fn(msg, MPIMessageTag::RequestRelRc as i32)
            }
            MPIMessage::Done => $self.0.$send_fn(&MPIDone(true), MPIMessageTag::Done as i32),
        }
    };
}

impl<'a, T: Source + Destination, N: Equivalence, E: Equivalence> MPISendRecv<N, E>
    for MPIStandardSendRecv<'a, T>
{
    type ReceiveOut = Ready<MPIMessage<N, E>>;

    fn send(&self, msg: &MPIMessage<N, E>) {
        generate_send_match!(self, msg, send_with_tag);
    }

    fn receive(&self) -> Self::ReceiveOut {
        let (msg, status) = self.0.matched_probe();
        future::ready(extract_message(msg, status))
    }
}

impl<'a, T: Source + Destination, N: Equivalence, E: Equivalence> MPISendRecv<N, E>
    for MPIBufferedSendRecv<'a, T>
{
    type ReceiveOut = Ready<MPIMessage<N, E>>;

    fn send(&self, msg: &MPIMessage<N, E>) {
        generate_send_match!(self, msg, buffered_send_with_tag);
    }

    fn receive(&self) -> Self::ReceiveOut {
        let (msg, status) = self.0.matched_probe();
        future::ready(extract_message(msg, status))
    }
}

impl<'a, T: Source + Destination, N: Equivalence, E: Equivalence> MPISendRecv<N, E>
    for MPIAsyncSendRecv<'a, T>
{
    type ReceiveOut = ReceiveMessageFuture<'a, T, MPIMessage<N, E>>;

    fn send(&self, msg: &MPIMessage<N, E>) {
        // We currently don't support sending asynchronously.
        generate_send_match!(self, msg, send_with_tag);
    }

    fn receive(&self) -> Self::ReceiveOut {
        ReceiveMessageFuture {
            process: self.0,
            _phantom: PhantomData,
        }
    }
}

fn extract_message<N: Equivalence, E: Equivalence>(
    msg: mpi::point_to_point::Message,
    status: mpi::point_to_point::Status,
) -> MPIMessage<N, E> {
    let tag: MPIMessageTag = status.tag().try_into().expect("invalid message tag");
    match tag {
        MPIMessageTag::RelRc => {
            let (msg, _) = msg.matched_receive::<MPIRelRc>();
            MPIMessage::RelRc(msg)
        }
        MPIMessageTag::NodeWeight => {
            let (msg, _) = msg.matched_receive::<N>();
            MPIMessage::NodeWeight(msg)
        }
        MPIMessageTag::IncomingEdge => {
            let default_edge = MPIIncomingEdge { source_hash: 0 };
            let n_elems = status.count(MPIIncomingEdge::equivalent_datatype()) as usize;
            let mut contents = vec![default_edge; n_elems];
            msg.matched_receive_into(&mut contents);
            MPIMessage::IncomingEdge(contents)
        }
        MPIMessageTag::EdgeWeight => {
            let (msg, _) = msg.matched_receive::<E>();
            MPIMessage::EdgeWeight(msg)
        }
        MPIMessageTag::RequestRelRc => {
            let (msg, _) = msg.matched_receive::<MPIRequestRelRc>();
            MPIMessage::RequestRelRc(msg)
        }
        MPIMessageTag::Done => {
            msg.matched_receive::<MPIDone>();
            MPIMessage::Done
        }
    }
}

/// A future that probes for a new MPI message.
pub(super) struct ReceiveMessageFuture<'a, T, M> {
    process: &'a T,
    _phantom: PhantomData<M>,
}

impl<'a, T: Source + Destination, N: Equivalence, E: Equivalence> Future
    for ReceiveMessageFuture<'a, T, MPIMessage<N, E>>
{
    type Output = MPIMessage<N, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.process.immediate_matched_probe() {
            Some((msg, status)) => std::task::Poll::Ready(extract_message(msg, status)),
            None => {
                // Not ready yet, register waker and return Pending
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }
}
