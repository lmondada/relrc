//! Graphs of objects history.
//!
//! [`AncestorGraph`]s are views into a set of nodes and all their predecessors.
//! The easiest way to traverse these graphs is using the `petgraph` traits by
//! activating the `petgraph` feature of this crate.

#[cfg(feature = "serde")]
mod serde;

use std::collections::{BTreeMap, BTreeSet};

use crate::{node::InnerData, RelRc, RelWeak};
use std::hash::Hash;

use derive_more::{From, Into};
use derive_where::derive_where;
#[cfg(feature = "petgraph")]
use petgraph::visit::IntoEdges;
#[cfg(feature = "petgraph")]
use std::borrow::Borrow;

/// View the dependencies for a set of [`RelRc`]s as a graph.
///
/// Represents induced subgraphs of [`RelRc`] objects, with directed edges
/// representing parent-child relationships.
///
/// [`GraphView`] instances hold strong references to the leaves (sinks) of the
/// subgraph, thus guaranteeing that all nodes in the graph are alive at least
/// as long as the graph object.
///
/// Nodes are represented by [`NodeId`], which are copiable raw pointers to the data.
/// The data is guaranteed to exist as long as the [`GraphView`] exists.
/// Accessing invalid node IDs will result in undefined behaviour, and may
/// access arbitrary (unsafe!) memory addresses.
#[derive_where(Clone, Default)]
pub struct RelRcGraph<N, E> {
    /// The nodes with outdegree 0 in the graph.
    ///
    /// We maintain strong references to these nodes, guaranteeing that all
    /// the nodes in the graph remain in memory.
    ///
    /// All nodes in the graph are ancestors of one of these nodes.
    sinks: Vec<RelRc<N, E>>,
    /// The nodes that induce the graph.
    ///
    /// Guaranteed to be alive as we maintain strong references to the sinks.
    all_nodes: BTreeSet<NodeId<N, E>>,
}

impl<N, E> RelRcGraph<N, E> {
    /// Create the graph of all ancestors of `sinks`.
    pub fn from_sinks(sinks: Vec<RelRc<N, E>>) -> Self {
        Self::from_sinks_while(sinks, |_| true)
    }

    /// Create the graph of all ancestors of `sinks` that can be reached without
    /// traversing an object for which `condition` returns `false`.
    pub fn from_sinks_while(
        sinks: Vec<RelRc<N, E>>,
        condition: impl Fn(&RelRc<N, E>) -> bool,
    ) -> Self {
        let mut all_nodes: BTreeSet<NodeId<_, _>> = Default::default();
        let as_entry = |n: &'_ RelRc<N, E>| (RelRc::as_ptr(n).into(), n.clone());
        let mut curr_nodes: BTreeMap<_, _> = sinks.iter().map(as_entry).collect();

        while let Some((node_id, node)) = curr_nodes.pop_first() {
            if !all_nodes.contains(&node_id) && condition(&node) {
                all_nodes.insert(node_id);
                curr_nodes.extend(node.all_parents().map(as_entry));
            }
        }

        Self { sinks, all_nodes }
    }

    /// Create the descendants graph of all `sources`.
    ///
    /// This will keep strong references to the deepest [`RelRc`] objects alive
    /// at call time.
    pub fn from_sources(sources: BTreeSet<RelRc<N, E>>) -> Self {
        let mut all_nodes = BTreeSet::new();
        let mut sinks = Vec::new();
        let mut curr_nodes: BTreeMap<_, _> = sources
            .iter()
            .map(|n| (RelRc::as_ptr(n), n.clone()))
            .collect();

        while let Some((node_id, node)) = curr_nodes.pop_first() {
            if all_nodes.insert(node_id.into()) {
                let children: Vec<_> = node.all_children().collect();
                if children.is_empty() {
                    sinks.push(node);
                } else {
                    curr_nodes.extend(children.into_iter().map(|n| (RelRc::as_ptr(&n), n)));
                }
            }
        }

        Self { sinks, all_nodes }
    }

    /// Merge two ancestor graphs.
    ///
    /// The resulting graph will contain all nodes from both graphs.
    pub fn merge(&mut self, other: Self) {
        self.sinks.extend(other.sinks);
        self.all_nodes.extend(other.all_nodes);

        // Make sure all terminal nodes are unique
        self.sinks.sort_by_key(|node| RelRc::as_ptr(node));
        self.sinks.dedup_by_key(|node| RelRc::as_ptr(node));
    }

    /// Find the lowest common ancestors of two graphs.
    ///
    /// The lowest common ancestor of two nodes is the deepest node that is an
    /// ancestor of both nodes.
    #[cfg(feature = "petgraph")]
    pub fn lowest_common_ancestors<'a>(
        graphs: &'a [impl Borrow<Self>],
    ) -> impl Iterator<Item = NodeId<N, E>> + 'a
    where
        N: 'a,
        E: 'a,
    {
        let node_indices = all_indices(graphs.iter().map(|g| g.borrow().all_nodes.iter().copied()));

        // Find all nodes that are in at least two graphs...
        node_indices
            .into_iter()
            .filter(|(_, indices)| indices.len() >= 2)
            .filter(|(n, indices)| {
                let edge_indices = all_indices(indices.iter().map(|i| {
                    let graph = &graphs[*i].borrow();
                    graph.edges(*n)
                }));
                // ...and with at least one outgoing edge not in all graphs
                edge_indices
                    .into_iter()
                    .any(|(_, e_indices)| e_indices.len() < indices.len())
            })
            .map(|(n, _)| n)
    }

    /// The nodes in the ancestor graph with outdegree 0.
    pub fn sinks(&self) -> &[RelRc<N, E>] {
        &self.sinks
    }

    /// Get all nodes in the ancestor graph.
    pub fn all_nodes(&self) -> &BTreeSet<NodeId<N, E>> {
        &self.all_nodes
    }

    /// Get the node data for a node identifier.
    ///
    /// This has undefined behaviour if the node identifier is invalid.
    pub fn get_node(&self, node_id: NodeId<N, E>) -> &InnerData<N, E> {
        unsafe { &*node_id.0 }
    }

    /// Get a [`RelRc`] to the node data for a node identifier.
    pub fn get_node_rc(&self, node_id: NodeId<N, E>) -> RelRc<N, E> {
        let node = self.get_node(node_id);
        if let Some(out_edge) = node.all_outgoing().first() {
            out_edge.source().clone()
        } else {
            // must be a terminal node
            self.sinks
                .iter()
                .find(|node| RelRc::as_ptr(node) == node_id.0)
                .expect("invalid node id: neither internal nor a terminal node")
                .clone()
        }
    }
}

#[cfg(feature = "petgraph")]
fn all_indices<I: IntoIterator>(items: impl IntoIterator<Item = I>) -> BTreeMap<I::Item, Vec<usize>>
where
    I::Item: Ord,
{
    let mut counts = BTreeMap::new();
    for (i, item) in items.into_iter().enumerate() {
        for node in item {
            counts.entry(node).or_insert(vec![]).push(i);
        }
    }
    counts
}

/// A node identifier in an [`AncestorGraph`].
#[derive(From, Into)]
pub struct NodeId<N, E>(pub(crate) *const InnerData<N, E>);

impl<N, E> std::fmt::Debug for NodeId<N, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "NodeId({:?})", self.0)
    }
}

impl<N, E> PartialEq for NodeId<N, E> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<N, E> Eq for NodeId<N, E> {}

impl<N, E> PartialOrd for NodeId<N, E> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<N, E> Ord for NodeId<N, E> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<N, E> Copy for NodeId<N, E> {}

impl<N, E> Clone for NodeId<N, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<N, E> Hash for NodeId<N, E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<'a, N, E> From<&'a RelRc<N, E>> for NodeId<N, E> {
    fn from(node: &'a RelRc<N, E>) -> Self {
        Self(RelRc::as_ptr(node))
    }
}

impl<'a, N, E> From<&'a RelWeak<N, E>> for NodeId<N, E> {
    fn from(node: &'a RelWeak<N, E>) -> Self {
        Self(RelWeak::as_ptr(node))
    }
}

/// An edge identifier in an [`AncestorGraph`].
///
/// The edge is uniquely identified by the edge target and the index of the
/// incoming edge at the target.
pub struct EdgeId<N, E> {
    /// The edge target.
    pub(crate) target: NodeId<N, E>,
    /// The incoming index of the edge at the target.
    pub(crate) index: usize,
}

impl<N, E> std::fmt::Debug for EdgeId<N, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EdgeId({:?}, {})", self.target, self.index)
    }
}

impl<N, E> Clone for EdgeId<N, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<N, E> Copy for EdgeId<N, E> {}

impl<N, E> PartialEq for EdgeId<N, E> {
    fn eq(&self, other: &Self) -> bool {
        self.target == other.target && self.index == other.index
    }
}

impl<N, E> Eq for EdgeId<N, E> {}

impl<N, E> PartialOrd for EdgeId<N, E> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<N, E> Ord for EdgeId<N, E> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.target
            .cmp(&other.target)
            .then(self.index.cmp(&other.index))
    }
}
