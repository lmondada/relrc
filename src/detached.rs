//! Attach and detach [`RelRc`] objects for serialization and transfer between
//! parents.
//!
//! Every [`RelRc`] object can be turned from an in-memory ref-counted pointer
//! object to a serializable object. This object can then be transferred (to a
//! potentially different process or machine), where it can be re-attached to
//! other [`RelRc`] objects.

#[cfg(feature = "mpi")]
mod mpi;

#[cfg(feature = "mpi")]
pub use mpi::{MPIRecvRelRc, MPISendRelRc};

use std::collections::{BTreeMap, BTreeSet};
use std::hash::Hash;

use crate::{edge::InnerEdgeData, hash_id::RelRcHash, node::InnerData, RelRc};
use itertools::Itertools;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// A detached object, obtained from [`RelRc::detach`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Detached<N, E> {
    current: RelRcHash,
    all_data: BTreeMap<RelRcHash, DetachedInnerData<N, E>>,
}

impl<N: Clone, E: Clone> RelRc<N, E> {
    /// Detach the object from the graph.
    ///
    /// The set of [`RelRc`] `detach_from` specifies the objects that will
    /// be available when the object is re-attached.
    pub fn detach(&self, detach_from: &BTreeSet<RelRcHash>) -> Detached<N, E> {
        Detached::new(self, detach_from)
    }

    /// Attach a detached object to a new graph.
    ///
    /// The set of [`RelRc`] `attach_to` specifies the objects to attach
    /// the detached object to.
    ///
    /// Panics if not all objects that are required to attach the detached object
    /// are available in `attach_to`. Use [`Detached::attaches_to`] to check
    /// whether the attachment will succeed.
    pub fn attach(
        detached: Detached<N, E>,
        attach_to: impl IntoIterator<Item = RelRc<N, E>>,
    ) -> Self
    where
        N: Hash,
        E: Hash,
    {
        let attach_to: BTreeMap<RelRcHash, RelRc<N, E>> =
            attach_to.into_iter().map(|n| (n.hash_id(), n)).collect();

        if attach_to.contains_key(&detached.current) {
            return attach_to.get(&detached.current).unwrap().clone();
        }

        let mut all_new_relrc: BTreeMap<RelRcHash, RelRc<N, E>> = BTreeMap::new();

        post_order_for_each(
            // Start with the current object
            detached.current,
            // Populate all_new_relrc in post-order
            |id| {
                let data = detached.all_data.get(&id).cloned().unwrap();
                let parents = data.incoming.into_iter().map(|(parent, edge_value)| {
                    // All parents must already be in all_new_relrc or attach_to
                    let relrc = all_new_relrc
                        .get(&parent)
                        .or_else(|| attach_to.get(&parent))
                        .cloned()
                        .unwrap();
                    (relrc, edge_value)
                });
                // Create the new RelRc object
                let relrc = RelRc::with_parents(data.value, parents);
                all_new_relrc.insert(id, relrc);
            },
            // The successors of an object are its parents
            |id| {
                let data = detached.all_data.get(&id).unwrap();
                data.incoming.iter().map(|(parent, _)| *parent)
            },
            // Only visit objects that are not yet attached
            |id| !attach_to.contains_key(&id),
        );

        all_new_relrc.remove(&detached.current).unwrap()
    }
}

impl<N: Clone, E: Clone> Detached<N, E> {
    /// Create a new [`Detached`] object from a [`RelRc`] object.
    pub fn new(obj: &RelRc<N, E>, detach_from: &BTreeSet<RelRcHash>) -> Self {
        let current = obj.hash_id();
        let all_relrc = ancestors_upto(obj, detach_from);
        let all_data = all_relrc
            .into_iter()
            .map(|(id, n)| {
                let data = DetachedInnerData::new(n.value().clone(), n.all_incoming().to_vec());
                (id, data)
            })
            .collect();
        Self { current, all_data }
    }
}

impl<N, E> Detached<N, E> {
    /// Create an empty [`Detached`] object.
    ///
    /// Useful for building a [`Detached`] object incrementally.
    ///
    /// This constructor is not exported, we only want users to create
    /// [`Detached`] objects by detaching [`RelRc`] objects.
    #[cfg(feature = "mpi")]
    fn empty(current: RelRcHash) -> Self {
        Self {
            current,
            all_data: BTreeMap::new(),
        }
    }

    /// Get the number of ancestors of the detached object (including self).
    pub fn n_ancestors(&self) -> usize {
        self.all_data.len()
    }
}

impl<N, E> Detached<N, E> {
    /// Get the hashes of the objects that are required to successfully attach
    /// self.
    pub fn required_hashes(&self) -> impl Iterator<Item = RelRcHash> + '_ {
        self.all_data
            .values()
            .flat_map(|data| data.parents())
            .filter(|hash| !self.all_data.contains_key(hash))
            .unique()
    }

    /// Check if trying to attach `self` to the objects in `attach_to` will
    /// succeed.
    ///
    /// In other words, check that all objects that are required to attach `self`
    /// are available in `attach_to`.
    pub fn attaches_to(&self, attach_to: &BTreeMap<RelRcHash, RelRc<N, E>>) -> bool {
        self.required_hashes()
            .all(|hash| attach_to.contains_key(&hash))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub(crate) struct DetachedInnerData<N, E> {
    /// The value of the [`RelRc`] object.
    value: N,
    /// The incoming edges to the object.
    ///
    /// The edges are given by the hash of the source object and the edge value.
    incoming: Vec<(RelRcHash, E)>,
}

impl<'a, N, E> From<&'a InnerData<N, E>> for DetachedInnerData<&'a N, &'a E> {
    fn from(value: &'a InnerData<N, E>) -> Self {
        DetachedInnerData {
            value: value.value(),
            incoming: value
                .all_incoming()
                .iter()
                .map(|e| (e.source().hash_id(), e.value()))
                .collect(),
        }
    }
}

impl<N, E> DetachedInnerData<N, E> {
    fn new(value: N, incoming: Vec<InnerEdgeData<N, E>>) -> Self {
        DetachedInnerData {
            value,
            incoming: incoming
                .into_iter()
                .map(|InnerEdgeData { value, source, .. }| (source.hash_id(), value))
                .collect(),
        }
    }

    fn parents(&self) -> impl Iterator<Item = RelRcHash> + '_ {
        self.incoming.iter().map(|(hash, _)| *hash)
    }
}

fn ancestors_upto<N, E>(
    obj: &RelRc<N, E>,
    detach_from: &BTreeSet<RelRcHash>,
) -> BTreeMap<RelRcHash, RelRc<N, E>> {
    let mut all_nodes: BTreeMap<RelRcHash, RelRc<N, E>> = Default::default();
    let mut stack: Vec<_> = vec![obj.clone()];

    while let Some(node) = stack.pop() {
        let node_id = node.hash_id();
        if !all_nodes.contains_key(&node_id) && !detach_from.contains(&node_id) {
            stack.extend(node.all_parents().cloned());
            all_nodes.insert(node_id, node);
        }
    }

    all_nodes
}

fn post_order_for_each<V: Ord + Copy, I: IntoIterator<Item = V>>(
    start: V,
    mut f: impl FnMut(V),
    successors: impl Fn(V) -> I,
    visit: impl Fn(V) -> bool,
) {
    enum DfsEvent<V> {
        DfsEnter(V),
        DfsExit(V),
    }

    let mut stack = vec![DfsEvent::DfsEnter(start)];
    let mut visited = BTreeSet::new();
    while let Some(event) = stack.pop() {
        match event {
            DfsEvent::DfsEnter(v) => {
                stack.push(DfsEvent::DfsExit(v));
                for u in successors(v) {
                    if visit(u) && visited.insert(u) {
                        stack.push(DfsEvent::DfsEnter(u));
                    }
                }
            }
            DfsEvent::DfsExit(v) => {
                f(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::*;
    use crate::RelRc;
    use std::collections::BTreeSet;

    #[test]
    fn test_detach_and_attach_diamond() {
        // Create the first set of RelRc objects forming a diamond pattern
        let root1 = RelRc::new("A");
        let left_child1 = RelRc::with_parents("B", vec![(root1.clone(), "left")]);
        let right_child1 = RelRc::with_parents("C", vec![(root1.clone(), "right")]);
        let grandchild1 = RelRc::with_parents(
            "D",
            vec![
                (left_child1.clone(), "left"),
                (right_child1.clone(), "right"),
            ],
        );

        // Create the second set of RelRc objects
        let root2 = RelRc::new("A");
        let left_child2 = RelRc::with_parents("B", vec![(root2.clone(), "left")]);

        // Detach the grandchild from the first set
        let detach_from: BTreeSet<RelRcHash> =
            BTreeSet::from_iter([root1.hash_id(), left_child1.hash_id()]);

        let detached = grandchild1.detach(&detach_from);
        assert_eq!(detached.all_data.len(), 2);

        // Drop the grandchild to release the memory
        drop(grandchild1);
        assert_eq!(left_child1.all_children().len(), 0);
        assert_eq!(right_child1.all_children().len(), 0);

        // Attach the detached grandchild to the second set
        let attach_to = [root2.clone(), left_child2.clone()];
        let grandchild2 = RelRc::attach(detached.clone(), attach_to);

        // Verify that the grandchild is now attached to the second set
        assert_eq!(grandchild2.value(), &"D");
        assert_eq!(
            grandchild2.all_parents().map(|n| n.value()).collect_vec(),
            vec![&"B", &"C"]
        );
        assert_eq!(root2.all_children().len(), 2);

        // Verify that detaching grandchild2 yields the original detached object
        let detached2 = grandchild2.detach(&BTreeSet::from_iter([
            root2.hash_id(),
            left_child2.hash_id(),
        ]));
        assert_eq!(detached, detached2);
    }
}
