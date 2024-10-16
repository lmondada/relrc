use std::{
    future::{self, Future, Ready},
    marker::PhantomData,
};

use mpi::traits::{Destination, Equivalence, Source};

use super::msg_types::{MPIAck, MPIIncomingEdge, MPIMessage, MPIMessageTag, MPIRelRc};

/// Internal trait capturing the send and receive functionality for MPI
/// communication.
///
/// Generalises over the different MPI modes (standard, buffered, async).
pub(super) trait MPISendRecv<N, E> {
    type ReceiveOut: Future<Output = MPIMessage<N, E>>;

    /// Send a message.
    fn send(&self, msg: &MPIMessage<N, E>);

    /// Receive a message with the given tag.
    ///
    /// The type returned by the future is guaranteed to correspond to the tag
    /// passed as argument.
    fn receive(&self, tag: MPIMessageTag) -> Self::ReceiveOut;
}

/// Send and receive MPI messages using standard communication.
pub(super) struct MPIStandardSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

/// Send and receive MPI messages using buffered communication.
pub(super) struct MPIBufferedSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

/// Send and receive MPI messages using asynchronous communication.
pub(super) struct MPIAsyncSendRecv<'a, T: Source + Destination>(pub(super) &'a T);

/// Massage the MPIMessage into the appropriate MPI message type and send it
/// using `$send_fn`.
macro_rules! generate_send_match {
    ($self:expr, $msg:expr, $send_fn:ident) => {
        let tag = $msg.tag();
        match $msg {
            &MPIMessage::RelRc(hash) => {
                let msg = MPIRelRc { hash: hash.into() };
                $self.0.$send_fn(&msg, tag as i32)
            }
            MPIMessage::NodeWeight(node_weight) => $self.0.$send_fn(node_weight, tag as i32),
            MPIMessage::IncomingEdge(incoming_edges) => {
                let msg = incoming_edges
                    .iter()
                    .map(|&h| MPIIncomingEdge {
                        source_hash: h.into(),
                    })
                    .collect::<Vec<_>>();
                $self.0.$send_fn(&msg, tag as i32)
            }
            MPIMessage::EdgeWeight(edge_weight) => $self.0.$send_fn(edge_weight, tag as i32),
            &MPIMessage::RequestRelRc(hash) => {
                let msg = MPIAck { hash: hash.into() };
                $self.0.$send_fn(&msg, tag as i32)
            }
            MPIMessage::Done => $self.0.$send_fn(&0, tag as i32),
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

    fn receive(&self, tag: MPIMessageTag) -> Self::ReceiveOut {
        let (msg, status) = self.0.matched_probe_with_tag(tag as i32);
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

    fn receive(&self, tag: MPIMessageTag) -> Self::ReceiveOut {
        let (msg, status) = self.0.matched_probe_with_tag(tag as i32);
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

    fn receive(&self, tag: MPIMessageTag) -> Self::ReceiveOut {
        ReceiveMessageFuture {
            process: self.0,
            tag,
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
            msg.into()
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
            contents.into()
        }
        MPIMessageTag::EdgeWeight => {
            let (msg, _) = msg.matched_receive::<E>();
            MPIMessage::EdgeWeight(msg)
        }
        MPIMessageTag::Ack => {
            let (msg, _) = msg.matched_receive::<MPIAck>();
            msg.into()
        }
    }
}

/// A future that probes for a new MPI message.
pub(super) struct ReceiveMessageFuture<'a, T, M> {
    process: &'a T,
    tag: MPIMessageTag,
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
        match self
            .process
            .immediate_matched_probe_with_tag(self.tag as i32)
        {
            Some((msg, status)) => std::task::Poll::Ready(extract_message(msg, status)),
            None => {
                // Not ready yet, register waker and return Pending
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        }
    }
}
