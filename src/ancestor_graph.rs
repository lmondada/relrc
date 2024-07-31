//! Graphs of objects history.
//!
//! [`AncestorGraph`]s are views into a set of nodes and all their predecessors.
//! The easiest way to traverse these graphs is using the `petgraph` traits by
//! activating the `petgraph` feature of this crate.
use std::collections::BTreeSet;

use crate::{node::InnerData, RelRc};

use derive_more::{From, Into};

/// Graph of all ancestors of nodes in `terminal_nodes`.
///
/// It is sufficient to hold Rcs of the sinks, as all its ancestors are
/// guaranteed to remain alive as long as they are alive.
///
/// [`NodeId`] are copiable and are represented by raw pointers to the data.
/// The data is guaranteed to exist as long as the [`AncestorGraph`] exists.
/// Accessing invalid node IDs will result in undefined behaviour, and may
/// access arbitrary (unsafe!) memory addresses.
pub struct AncestorGraph<N, E> {
    /// The nodes with indegree 0.
    ///
    /// All nodes in the graph must be decendants of one of these nodes.
    initial_nodes: BTreeSet<NodeId<N, E>>,
    /// The nodes with outdegree 0 in the ancestor graph.
    ///
    /// We maintain strong references to these nodes, guaranteeing that all
    /// the nodes in the graph remain in memory.
    ///
    /// The graph is defined as the ancestors of these nodes.
    terminal_nodes: Vec<RelRc<N, E>>,
    /// The nodes in the ancestor graph.
    ///
    /// Guaranteed to be alive as we maintain strong references to the sinks.
    all_nodes: BTreeSet<NodeId<N, E>>,
}

impl<N, E> AncestorGraph<N, E> {
    /// Create the ancestor graph of all `terminal_nodes`.
    pub fn from_terminals(terminal_nodes: Vec<RelRc<N, E>>) -> Self {
        let mut all_nodes = BTreeSet::new();
        let mut initial_nodes = BTreeSet::new();
        let mut curr_nodes: Vec<_> = terminal_nodes.iter().map(RelRc::as_ptr).collect();

        while let Some(node_id) = curr_nodes.pop() {
            if all_nodes.insert(node_id.into()) {
                let node = unsafe { &*node_id };
                curr_nodes.extend(node.all_parents().map(RelRc::as_ptr));
                if node.n_incoming() == 0 {
                    initial_nodes.insert(node_id.into());
                }
            }
        }

        Self {
            initial_nodes,
            terminal_nodes,
            all_nodes,
        }
    }

    /// The nodes in the ancestor graph with indegree 0.
    pub fn initial_nodes(&self) -> &BTreeSet<NodeId<N, E>> {
        &self.initial_nodes
    }

    /// The nodes in the ancestor graph with outdegree 0.
    pub fn terminal_nodes(&self) -> &[RelRc<N, E>] {
        &self.terminal_nodes
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
            self.terminal_nodes
                .iter()
                .find(|node| RelRc::as_ptr(node) == node_id.0)
                .expect("invalid node id: neither internal nor a terminal node")
                .clone()
        }
    }
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

impl<'a, N, E> From<&'a RelRc<N, E>> for NodeId<N, E> {
    fn from(node: &'a RelRc<N, E>) -> Self {
        Self(RelRc::as_ptr(node))
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
