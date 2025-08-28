//! Dependencies between [`RelRc`] objects, presented as a directed graph.
//!
//! The graphs can be traversed using the provided APIs or using the `petgraph`
//! traits, by activating the `petgraph` feature of this crate.

use std::{cell::RefCell, rc::Rc};

use crate::{edge::InnerEdgeData, Edge, NodeId, Registry, RelRc};

use derive_more::{From, Into};
use derive_where::derive_where;
use slotmap_fork_lmondada::SecondaryMap;

/// A graph of [`RelRc`] nodes and their dependencies.
///
/// The `HistoryGraph` is designed to manage and traverse the data dependency
/// graph of `RelRc` nodes, obtained from the parent-child relationships
/// between `RelRc` nodes.
///
/// [`RelRc`] objects must be assigned to copyable IDs using a [`Registry`].
#[derive(Debug)]
#[derive_where(Clone, Default)]
pub struct HistoryGraph<N, E> {
    /// The nodes of the graph
    nodes: SecondaryMap<NodeId, RelRc<N, E>>,
    /// The map between relrc nodes and node IDs.
    registry: Rc<RefCell<Registry<N, E>>>,
}

impl<N, E> HistoryGraph<N, E> {
    /// Create a new [`HistoryGraph`] from a set of nodes..
    pub fn from_nodes(nodes: impl IntoIterator<Item = RelRc<N, E>>) -> Self {
        Self::new(nodes, Registry::new())
    }

    /// Create a new [`HistoryGraph`] with a [`Registry`].
    pub fn with_registry(registry: impl Into<Rc<RefCell<Registry<N, E>>>>) -> Self {
        Self::new([], registry)
    }

    /// Create a new [`HistoryGraph`] from a set of nodes and a registry.
    pub fn new(
        nodes: impl IntoIterator<Item = RelRc<N, E>>,
        registry: impl Into<Rc<RefCell<Registry<N, E>>>>,
    ) -> Self {
        let mut ret = Self {
            nodes: Default::default(),
            registry: registry.into(),
        };

        for node in nodes {
            ret.insert_node(node);
        }

        ret
    }

    /// Get all outgoing edge IDs from a node.
    pub fn outgoing_edges(&self, node_id: NodeId) -> impl Iterator<Item = EdgeId> + '_ {
        let source = self.get_node(node_id);
        let edges = source.map(|n| n.all_outgoing());
        let map_node_id = |Edge { target, index }| {
            self.get_node_id(&target)
                .map(|target| EdgeId { target, index })
        };
        edges.into_iter().flatten().filter_map(map_node_id)
    }

    /// Get all incoming edge IDs from a node.
    pub fn incoming_edges(&self, node_id: NodeId) -> impl Iterator<Item = EdgeId> + '_ {
        let target = self.get_node(node_id);
        let n_incoming = target.map(|n| n.n_incoming()).unwrap_or_default();
        (0..n_incoming)
            .map(move |index| EdgeId {
                target: node_id,
                index,
            })
            .filter(|&e| self.contains_edge(e))
    }

    /// Get all parent node IDs of a node.
    pub fn parents(&self, node_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.incoming_edges(node_id)
            .filter_map(|edge_id| self.source(edge_id))
    }

    /// Get all child node IDs of a node.
    pub fn children(&self, node_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing_edges(node_id)
            .filter_map(|edge_id| self.target(edge_id))
    }

    /// Get the registry of the history graph.
    pub fn registry(&self) -> &Rc<RefCell<Registry<N, E>>> {
        &self.registry
    }

    fn get_node_id(&self, node: &RelRc<N, E>) -> Option<NodeId> {
        let id = self.registry.borrow().get_id(node)?;
        self.nodes.contains_key(id).then_some(id)
    }

    /// Get all nodes in the ancestor graph.
    pub fn all_node_ids(&self) -> impl Iterator<Item = NodeId> + Clone + '_ {
        self.nodes.keys()
    }

    /// Check if a node is in the history graph.
    pub fn contains(&self, node: &RelRc<N, E>) -> bool {
        let Some(id) = self.registry.borrow().get_id(node) else {
            return false;
        };
        self.contains_id(id)
    }

    /// Check if a node id is in the history graph.
    pub fn contains_id(&self, node_id: NodeId) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Get the node data for a node identifier.
    ///
    /// Panic if the node id is invalid.
    pub fn get_node(&self, node_id: NodeId) -> Option<&RelRc<N, E>> {
        self.nodes.get(node_id)
    }

    /// Get the edge data for an edge identifier.
    ///
    /// Panic if the edge id is invalid.
    pub fn get_edge(&self, edge_id: EdgeId) -> Option<&InnerEdgeData<N, E>> {
        self.get_node(edge_id.target)?
            .incoming(edge_id.index)
            .filter(|e| self.contains(e.source()))
    }

    /// Check if an edge is in the history graph.
    pub fn contains_edge(&self, edge_id: EdgeId) -> bool {
        self.get_edge(edge_id).is_some()
    }

    /// Get the source node id of an edge.
    pub fn source(&self, edge_id: EdgeId) -> Option<NodeId> {
        let edge = self.get_edge(edge_id)?;
        let source_node = edge.source();
        self.get_node_id(source_node)
    }

    /// Get the target node id of an edge.
    pub fn target(&self, edge_id: EdgeId) -> Option<NodeId> {
        self.contains_edge(edge_id).then_some(edge_id.target)
    }

    /// Add a `RelRc` node to the `HistoryGraph`.
    ///
    /// Returns the `NodeId` of the added node. This may fail if the node is
    /// already registered in a different registry.
    ///
    /// # Arguments
    ///
    /// * `node` - The `RelRc` node to be added to the graph.
    ///
    /// # Returns
    ///
    /// * `NodeId` - The identifier of the added node.
    pub fn insert_node(&mut self, node: RelRc<N, E>) -> Option<NodeId> {
        if let Some(id) = self.get_node_id(&node) {
            return Some(id); // Node already exists, return its ID
        }
        let id = node.try_register_in(&self.registry)?;

        self.nodes.insert(id, node);
        Some(id)
    }

    /// Insert `node` and all its ancestors.
    ///
    /// This will panic if `node` or any of its ancestors are already registered
    /// in a different registry.
    pub fn insert_ancestors(&mut self, node: RelRc<N, E>) -> NodeId {
        for parent in node.all_parents() {
            if !self.contains(parent) {
                self.insert_ancestors(parent.clone());
            }
        }
        self.insert_node(node.clone()).expect("node not registered")
    }
}

/// An edge identifier in a [`RelRcGraph`].
///
/// The edge is uniquely identified by the edge target and the index of the
/// incoming edge at the target.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, From, Into)]
pub struct EdgeId {
    /// The edge target.
    pub target: NodeId,
    /// The incoming index of the edge at the target.
    pub index: usize,
}

#[cfg(test)]
mod tests {
    #![cfg(feature = "serde")]
    use insta::assert_yaml_snapshot;
    use itertools::Itertools;

    use super::*;
    use crate::RelRc;

    #[test]
    fn test_history_graph() {
        // Create first family tree
        let grandparent = RelRc::new((1, 10)); // First element 1 for equivalence
        let parent = RelRc::with_parents((1, 20), vec![(grandparent.clone(), 10)]);
        let child1 = RelRc::with_parents((2, 30), vec![(parent.clone(), 20)]);

        let child2 = RelRc::with_parents((3, 35), vec![(parent.clone(), 25)]);
        assert_eq!(child2.incoming(0).unwrap().value(), &25);

        // Create history graph with first family
        let mut graph = HistoryGraph::default();
        let child1_id = graph.insert_ancestors(child1);

        // Verify first family was added correctly
        let node_ids: Vec<_> = graph.all_node_ids().collect();
        assert_eq!(
            node_ids.len(),
            3,
            "Should have 3 nodes after adding first family"
        );

        // Check parent-child relationships in graph
        let parent_id = graph
            .source(EdgeId {
                target: child1_id,
                index: 0,
            })
            .unwrap();
        let grandparent_id = graph
            .source(EdgeId {
                target: parent_id,
                index: 0,
            })
            .unwrap();
        let out_grandparent = graph
            .outgoing_edges(grandparent_id)
            .exactly_one()
            .ok()
            .unwrap();
        assert_eq!(out_grandparent.target, parent_id);
        let out_parent = graph.outgoing_edges(parent_id).exactly_one().ok().unwrap();
        assert_eq!(out_parent.target, child1_id);

        // Add second family
        let child2_id = graph.insert_ancestors(child2);

        // Verify only one new node was added (child2)
        let node_ids: Vec<_> = graph.all_node_ids().sorted().collect();
        assert_eq!(
            node_ids.len(),
            4,
            "Should have 4 nodes after adding second family"
        );
        assert_eq!(node_ids, [grandparent_id, parent_id, child1_id, child2_id]);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn test_history_graph_serialization() {
        // Create first family tree

        use std::collections::BTreeSet;

        let grandparent1 = RelRc::new((1, 10)); // First element 1 for equivalence
        let parent1 = RelRc::with_parents((1, 20), vec![(grandparent1.clone(), 10)]);
        let child1 = RelRc::with_parents((2, 30), vec![(parent1.clone(), 20)]);

        // Create second family tree with equivalent parents but distinct child
        let grandparent2 = RelRc::new((1, 15)); // First element 1 for equivalence
        let parent2 = RelRc::with_parents((1, 25), vec![(grandparent2.clone(), 15)]);
        let child2 = RelRc::with_parents((3, 35), vec![(parent2.clone(), 25)]);
        assert_eq!(child2.incoming(0).unwrap().value(), &25);

        // Create history graph with first family
        let mut graph = HistoryGraph::default();
        graph.insert_ancestors(child1);
        graph.insert_ancestors(child2);

        let serialized = graph.to_serialized();
        assert_yaml_snapshot!(serialized);

        let deserialized = HistoryGraph::from_serialized(serialized);

        assert_eq!(
            graph.all_node_ids().collect::<BTreeSet<_>>(),
            deserialized.all_node_ids().collect::<BTreeSet<_>>()
        );
    }
}
