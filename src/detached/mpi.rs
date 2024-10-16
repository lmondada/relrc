//! A simple protocol for transferring [`RelRc`] objects between processes using
//! MPI.
//!
//! The protocol will start by sending the object to be transmitted without
//! specifying any of its ancestors. The receiver may then request the transfer
//! of any of its ancestors that it does not have yet. This will continue until
//! all ancestors have been transferred and the [`RelRc`] object can successfully
//! be attached in the receiver process.

use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::hash::Hash;

use futures::executor;
use itertools::Itertools;
use mpi::traits::{Destination, Equivalence, Source};
use msg_types::{MPIIncomingEdge, MPIMessage, MPIRelRc, MPIRequestRelRc};
use send_recv::{MPIAsyncSendRecv, MPIBufferedSendRecv, MPISendRecv, MPIStandardSendRecv};

use crate::{detached::Detached, hash_id::RelRcHash, RelRc};

use super::DetachedInnerData;

mod msg_types;
mod send_recv;

/// The mode of communication for transferring [`RelRc`] objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MPIMode {
    /// Standard MPI communication.
    ///
    /// Up to the MPI implementation to choose whether this is buffered or
    /// blocking, as well as the size of the buffer if applicable.
    #[default]
    Standard,
    /// Buffered MPI communication.
    ///
    /// Sends will be fast, but might run out of (user-managed) buffer.
    Buffered,
    /// Asynchronous MPI communication.
    ///
    /// Currently only supported for receiving [`RelRc`] objects.
    Async,
}

/// Transfer [`RelRc`] objects between processes using MPI.
///
/// Will send as many ancestors as necessary from the current process.
///
/// This is implemented for any type that implements [`mpi::traits::Source`] and
/// [`mpi::traits::Destination`].
pub trait RelRcCommunicator<N, E> {
    /// Send a [`RelRc`] to another process.
    fn send_relrc(&self, relrc: &RelRc<N, E>, mode: MPIMode) {
        executor::block_on(self.send_relrc_async(relrc, mode))
    }

    /// Receive a [`RelRc`] from another process.
    fn recv_relrc(
        &self,
        attach_to: impl IntoIterator<Item = RelRc<N, E>>,
        mode: MPIMode,
    ) -> RelRc<N, E> {
        if mode == MPIMode::Async {
            panic!("Use recv_relrc_async instead of recv_relrc for async mode");
        }
        executor::block_on(self.recv_relrc_async(attach_to, mode))
    }

    /// Send a [`RelRc`] to another process returning a future.
    ///
    /// Note that sends themselves are not asynchronoous (mode == MPIMode::Async
    /// is currently not supported!). However, sending the data successfully may
    /// require several rounds of send-receive operations, so receives may run
    /// asynchronously.
    fn send_relrc_async(&self, relrc: &RelRc<N, E>, mode: MPIMode) -> impl Future<Output = ()>;

    /// Receive a [`RelRc`] from another process asynchronously.
    fn recv_relrc_async(
        &self,
        attach_to: impl IntoIterator<Item = RelRc<N, E>>,
        mode: MPIMode,
    ) -> impl Future<Output = RelRc<N, E>>;
}

impl<T, N, E> RelRcCommunicator<N, E> for T
where
    T: Source + Destination,
    N: Hash + Clone + Equivalence,
    E: Hash + Clone + Equivalence,
{
    async fn send_relrc_async(&self, relrc: &RelRc<N, E>, mode: MPIMode) {
        match mode {
            MPIMode::Buffered => {
                let dest = MPIBufferedSendRecv(self);
                send_relrc(&dest, relrc).await;
            }
            MPIMode::Standard => {
                let dest = MPIStandardSendRecv(self);
                send_relrc(&dest, relrc).await;
            }
            MPIMode::Async => {
                unimplemented!(
                    "Async mode not supported for sending. Use Standard or Buffered mode instead."
                );
            }
        }
    }

    async fn recv_relrc_async(
        &self,
        attach_to: impl IntoIterator<Item = RelRc<N, E>>,
        mode: MPIMode,
    ) -> RelRc<N, E> {
        // Cast self to the appropriate type based on the mode and call the
        // recv_relrc function
        macro_rules! recv_with_mode {
            ($mode:expr) => {{
                let source = $mode(self);
                recv_relrc(&source, attach_to).await
            }};
        }

        match mode {
            MPIMode::Buffered => recv_with_mode!(MPIBufferedSendRecv),
            MPIMode::Standard => recv_with_mode!(MPIStandardSendRecv),
            MPIMode::Async => recv_with_mode!(MPIAsyncSendRecv),
        }
    }
}

async fn send_relrc<N: Hash + Clone, E: Hash + Clone>(
    dest: &impl MPISendRecv<N, E>,
    relrc: &RelRc<N, E>,
) {
    // by leaving the set empty, we make no assumptions on what the receiver knows
    // Add stuff there to make this more efficient
    let detached = relrc.detach(&BTreeSet::new());

    mpi_send(
        dest,
        detached.current,
        &detached.all_data[&detached.current],
    );

    // Now wait for a confirmation or send further objects if requested
    loop {
        let msg = dest.receive().await;
        if matches!(msg, MPIMessage::Done) {
            break;
        }

        // Send the requested object
        let MPIMessage::RequestRelRc(MPIRequestRelRc { hash }) = msg else {
            panic!("Received unexpected message");
        };
        let hash = RelRcHash::from(hash);
        mpi_send(dest, hash, &detached.all_data[&hash]);
    }
}

async fn recv_relrc<N: Hash + Clone, E: Hash + Clone>(
    source: &impl MPISendRecv<N, E>,
    attach_to: impl IntoIterator<Item = RelRc<N, E>>,
) -> RelRc<N, E> {
    let attach_to: BTreeMap<RelRcHash, RelRc<N, E>> =
        attach_to.into_iter().map(|r| (r.hash_id(), r)).collect();

    let mut detached: Option<Detached<N, E>> = None;

    // While detached object is not ready to be attached
    while detached.is_none() || !detached.as_ref().unwrap().attaches_to(&attach_to) {
        if let Some(detached) = detached.as_ref() {
            // Request more objects
            let first_unknown_hash = detached
                .required_hashes()
                .find(|hash| !attach_to.contains_key(hash))
                .expect("cannot attach but all required objects are known");
            let msg = MPIRequestRelRc {
                hash: first_unknown_hash.into(),
            };
            source.send(&msg.into());
        }

        // Receive the object (either first time or just requested)
        let (hash, detached_inner) = mpi_recv(source).await;

        if detached.is_none() {
            detached = Some(Detached::empty(hash));
        }

        // Insert the received object into the detached data
        let all_data = &mut detached.as_mut().unwrap().all_data;
        all_data.insert(hash, detached_inner);
    }

    source.send(&MPIMessage::Done);

    RelRc::attach(detached.unwrap(), attach_to.values().cloned())
}

/// Send a single [`RelRc`] object to `dest` according to our protocol.
///
/// We don't return a promise as we currently only support blocking sends. These
/// should be fast as long as the buffer doesn't run out.
fn mpi_send<N: Clone, E: Clone>(
    dest: &impl MPISendRecv<N, E>,
    hash: RelRcHash,
    data: &DetachedInnerData<N, E>,
) {
    // 0. The RelRc message (we could send more than one at a time)
    let relrc_msg = MPIRelRc { hash: hash.into() };
    dest.send(&relrc_msg.into());

    // 1. All the node weights one-by-one (just one)
    dest.send(&MPIMessage::NodeWeight(data.value.clone()));

    let (incoming_hashes, incoming_values): (Vec<_>, Vec<_>) =
        data.incoming.iter().map(|(fst, snd)| (*fst, snd)).unzip();
    // 2. All the incoming edges all in a vec
    let msgs = incoming_hashes
        .into_iter()
        .map(|hash| MPIIncomingEdge {
            source_hash: hash.into(),
        })
        .collect_vec();
    dest.send(&msgs.into());

    // 3. The edge weights one-by-one
    for weight in incoming_values {
        dest.send(&MPIMessage::EdgeWeight(weight.clone()));
    }
}

/// Receive a single [`RelRc`] object from `source` according to our protocol.
async fn mpi_recv<N, E>(source: &impl MPISendRecv<N, E>) -> (RelRcHash, DetachedInnerData<N, E>) {
    // 0. Receive the RelRc message
    let MPIMessage::RelRc(msg) = source.receive().await else {
        panic!("Expected RelRc message");
    };
    let hash = RelRcHash::from(msg);

    // 1. Receive all the node weights (just one atm)
    let MPIMessage::NodeWeight(node_weight) = source.receive().await else {
        panic!("Expected node weight message");
    };

    // 2. Receive all the incoming edges
    let MPIMessage::IncomingEdge(incoming_edges) = source.receive().await else {
        panic!("Expected incoming edge message");
    };

    // 3. Receive all the edge weights
    let mut incoming = Vec::with_capacity(incoming_edges.len());
    for edge in incoming_edges {
        let source_hash = RelRcHash::from(edge.source_hash);
        let MPIMessage::EdgeWeight(edge_weight) = source.receive().await else {
            panic!("Expected edge weight message");
        };
        incoming.push((source_hash, edge_weight));
    }

    (
        hash,
        DetachedInnerData {
            value: node_weight,
            incoming,
        },
    )
}
