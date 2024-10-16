//! A simple protocol for transferring [`RelRc`] objects between processes using
//! MPI.
//!
//! The protocol will start by sending the object to be transmitted without
//! specifying any of its ancestors. The receiver may then request the transfer
//! of any of its ancestors that it does not have yet. This will continue until
//! all ancestors have been transferred and the [`RelRc`] object can successfully
//! be attached in the receiver process.

use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;

use itertools::Itertools;
use mpi::traits::{Destination, Equivalence, Source};
use mpi_types::{MPIDone, MPIIncomingEdge, MPIMessageTag, MPIRelRc, MPIRequestRelRc};

use crate::{detached::Detached, hash_id::RelRcHash, RelRc};

use super::DetachedInnerData;

mod mpi_types;

/// Send a [`RelRc`] to another process.
///
/// Will send as many ancestors as necessary from the current process.
pub trait MPISendRelRc<N, E> {
    /// Send a [`RelRc`] to another process.
    fn send_relrc(&self, relrc: &RelRc<N, E>);
}

/// Receive a [`RelRc`] from another process and attach it to the given objects.
pub trait MPIRecvRelRc<N, E> {
    /// Receive a [`RelRc`] from another process.
    fn recv_relrc(&self, attach_to: impl IntoIterator<Item = RelRc<N, E>>) -> RelRc<N, E>;
}

impl<T: Source + Destination, N: Equivalence + Clone, E: Equivalence + Clone> MPISendRelRc<N, E>
    for T
{
    fn send_relrc(&self, relrc: &RelRc<N, E>) {
        // by leaving the set empty, we make no assumptions on what the receiver knows
        // Add stuff there to make this more efficient
        let detached = relrc.detach(&BTreeSet::new());

        mpi_send(
            self,
            detached.current,
            &detached.all_data[&detached.current],
        );

        // Now wait for a confirmation or send further objects if requested
        loop {
            let (msg, status) = self.matched_probe();
            if status.tag() == MPIMessageTag::Done as i32 {
                msg.matched_receive::<MPIDone>();
                break;
            }

            assert_eq!(status.tag(), MPIMessageTag::RequestRelRc as i32);

            // Send the requested object
            let (MPIRequestRelRc { hash }, _) = msg.matched_receive::<MPIRequestRelRc>();
            let hash = RelRcHash::from(hash);
            mpi_send(self, hash, &detached.all_data[&hash]);
        }
    }
}

impl<T, N, E> MPIRecvRelRc<N, E> for T
where
    T: Source + Destination,
    N: Hash + Clone + Equivalence,
    E: Hash + Clone + Equivalence,
{
    fn recv_relrc(&self, attach_to: impl IntoIterator<Item = RelRc<N, E>>) -> RelRc<N, E> {
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
                self.send_with_tag(&msg, MPIMessageTag::RequestRelRc as i32);
            }

            // Receive the object (either first time or just requested)
            let (hash, detached_inner) = mpi_recv(self);

            if detached.is_none() {
                detached = Some(Detached::empty(hash));
            }

            // Insert the received object into the detached data
            let all_data = &mut detached.as_mut().unwrap().all_data;
            all_data.insert(hash, detached_inner);
        }

        self.send_with_tag(&MPIDone(true), MPIMessageTag::Done as i32);

        RelRc::attach(detached.unwrap(), attach_to.values().cloned())
    }
}

fn mpi_send<D: Destination, N: Equivalence, E: Equivalence>(
    dest: &D,
    hash: RelRcHash,
    data: &DetachedInnerData<N, E>,
) {
    // 0. The RelRc message (we could send more than one at a time)
    let relrc_msg = MPIRelRc { hash: hash.into() };
    dest.send_with_tag(&relrc_msg, MPIMessageTag::RelRc as i32);

    // 1. All the node weights one-by-one (just one)
    dest.send_with_tag(&data.value, MPIMessageTag::NodeWeight as i32);

    let (incoming_hashes, incoming_values): (Vec<_>, Vec<_>) =
        data.incoming.iter().map(|(fst, snd)| (*fst, snd)).unzip();
    // 2. All the incoming edges all in a vec
    let msgs = incoming_hashes
        .into_iter()
        .map(|hash| MPIIncomingEdge {
            source_hash: hash.into(),
        })
        .collect_vec();
    dest.send_with_tag(&msgs, MPIMessageTag::IncomingEdge as i32);

    // 3. The edge weights one-by-one
    for weight in incoming_values {
        dest.send_with_tag(weight, MPIMessageTag::EdgeWeight as i32);
    }
}

fn mpi_recv<N: Equivalence, E: Equivalence>(
    source: &impl Source,
) -> (RelRcHash, DetachedInnerData<N, E>) {
    // 0. Receive the RelRc message
    let (MPIRelRc { hash }, status) = source.receive::<MPIRelRc>();
    assert_eq!(status.tag(), MPIMessageTag::RelRc as i32);
    let hash = RelRcHash::from(hash);

    // 1. Receive all the node weights (just one atm)
    let (node_weight, status) = source.receive::<N>();
    assert_eq!(status.tag(), MPIMessageTag::NodeWeight as i32);

    // 2. Receive all the incoming edges
    let (incoming_edges, status) = source.receive_vec::<MPIIncomingEdge>();
    assert_eq!(status.tag(), MPIMessageTag::IncomingEdge as i32);

    // 3. Receive all the edge weights
    let mut incoming = Vec::with_capacity(incoming_edges.len());
    for edge in incoming_edges {
        let source_hash = RelRcHash::from(edge.source_hash);
        let (edge_weight, status) = source.receive::<E>();
        assert_eq!(status.tag(), MPIMessageTag::EdgeWeight as i32);
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
