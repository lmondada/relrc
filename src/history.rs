//! Graphs of objects history.
//!
//! [`AncestorGraph`]s are views into a set of nodes and all their predecessors.
//! The easiest way to traverse these graphs is using the `petgraph` traits by
//! activating the `petgraph` feature of this crate.

use fxhash::{FxHashMap, FxHasher64};
use itertools::{izip, Itertools};

use std::hash::{Hash, Hasher};

use crate::{
    edge::InnerEdgeData,
    node::InnerData,
    resolver::{EquivalenceResolver, ResolverId},
    Edge, RelRc,
};

use derive_more::{From, Into};
use derive_where::derive_where;
use std::fmt;
use std::str::FromStr;

/// Represents a Merkle Tree hash of all deduplication keys in the ancestor
/// graph of a `RelRc`.
///
/// This hash is used to uniquely identify nodes in the `HistoryGraph` by
/// considering not only the node's own deduplication key but also the keys of
/// all its ancestors. This ensures that nodes are only considered equivalent if
/// their entire history matches, allowing for accurate deduplication and
/// merging of semantically equivalent nodes.
type AncestorGraphHash = u64;

/// A graph of `RelRc` nodes and their dependencies.
///
/// The `HistoryGraph` is designed to manage and traverse the data dependency
/// graph of `RelRc` nodes, obtained from the parent-child relationships
/// between `RelRc` nodes.
///
/// A `HistoryGraph` that contains a [`crate::RelRc`] node will also always
/// contain  all its ancestors.
///
/// # Key Features
///
/// - **Graph Traversal**: Provides methods to traverse the graph, accessing
///   nodes and their relationships.
/// - **Deduplication**: Utilizes the `EquivalenceResolver` trait to identify
///   and merge semantically equivalent `RelRc` nodes. Internally, Merkle Tree
///   hashes are used to uniquely identify nodes based on their entire history.
#[derive(Debug)]
#[derive_where(Clone, Default; R)]
pub struct HistoryGraph<N, E, R> {
    /// A mapping from `AncestorGraphHash` to a list of `RelRc` nodes.
    ///
    /// This map stores all nodes in the `HistoryGraph`, grouped by their
    /// ancestor graph hash. It allows efficient lookup and management of
    /// nodes, facilitating the deduplication process by ensuring that nodes
    /// with the same ancestor graph hash are considered for merging.
    nodes_by_hash: FxHashMap<AncestorGraphHash, Vec<RelRc<N, E>>>,
    /// The resolver used to compare node and edge values for equivalence.
    resolver: R,
    /// Map data pointer of [`crate::RelRc`]s in the history graph to their node
    /// id.
    ///
    /// Not exposed in any way, but required to find the node IDs of e.g.
    /// all children or parents of a node.
    ptr_to_node_id: FxHashMap<*const InnerData<N, E>, NodeId>,
}

impl<N, E, R> HistoryGraph<N, E, R> {
    /// Create a new [`RelRcGraph`].
    pub fn with_resolver(resolver: R) -> Self {
        Self {
            nodes_by_hash: FxHashMap::default(),
            resolver,
            ptr_to_node_id: FxHashMap::default(),
        }
    }

    /// Get all outgoing edge IDs from a node.
    pub fn outgoing_edges(&self, node_id: NodeId) -> impl Iterator<Item = EdgeId> + '_ {
        let node = self.get_node(node_id);
        let edges = node.all_outgoing();
        let map_node_id = |Edge { target, index }| {
            self.get_node_id(&target)
                .map(|target| EdgeId { target, index })
        };
        edges.into_iter().filter_map(map_node_id)
    }

    /// Get all incoming edge IDs from a node.
    pub fn incoming_edges(&self, node_id: NodeId) -> impl Iterator<Item = EdgeId> + '_ {
        let node = self.get_node(node_id);
        (0..node.n_incoming()).map(move |index| EdgeId {
            target: node_id,
            index,
        })
    }

    /// Get all parent node IDs of a node.
    pub fn parents(&self, node_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.incoming_edges(node_id)
            .map(|edge_id| self.source(edge_id))
    }

    /// Get all child node IDs of a node.
    pub fn children(&self, node_id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        self.outgoing_edges(node_id)
            .map(|edge_id| self.target(edge_id))
    }

    fn get_node_id(&self, node: &RelRc<N, E>) -> Option<NodeId> {
        self.ptr_to_node_id.get(&node.as_ptr()).copied()
    }

    /// Get all nodes in the ancestor graph.
    pub fn all_node_ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.nodes_by_hash
            .iter()
            .flat_map(|(key, vec)| (0..vec.len()).map(move |i| NodeId(*key, i)))
    }

    /// Check if a node is in the history graph.
    pub fn contains(&self, node: &RelRc<N, E>) -> bool {
        self.get_node_id(node).is_some()
    }

    /// Check if a node id is in the history graph.
    pub fn contains_id(&self, node_id: NodeId) -> bool {
        self.nodes_by_hash
            .get(&node_id.0)
            .map_or(false, |v| v.len() > node_id.1)
    }

    /// Get the node data for a node identifier.
    ///
    /// Panic if the node id is invalid.
    pub fn get_node(&self, node_id: NodeId) -> &RelRc<N, E> {
        self.nodes_by_hash
            .get(&node_id.0)
            .and_then(|v| v.get(node_id.1))
            .expect("invalid node id")
    }

    /// Get the edge data for an edge identifier.
    ///
    /// Panic if the edge id is invalid.
    pub fn get_edge(&self, edge_id: EdgeId) -> &InnerEdgeData<N, E> {
        self.get_node(edge_id.target)
            .incoming(edge_id.index)
            .expect("invalid edge id")
    }

    /// Get the source node id of an edge.
    pub fn source(&self, edge_id: EdgeId) -> NodeId {
        let edge = self.get_edge(edge_id);
        let source_node = edge.source();
        self.get_node_id(&source_node)
            .expect("source must be in history")
    }

    /// Get the target node id of an edge.
    pub fn target(&self, edge_id: EdgeId) -> NodeId {
        edge_id.target
    }

    /// Get the resolver id of the history graph.
    pub fn resolver_id(&self) -> ResolverId<N, E, R>
    where
        R: EquivalenceResolver<N, E>,
    {
        (&self.resolver).into()
    }
}

impl<N: Clone, E: Clone, R: EquivalenceResolver<N, E>> HistoryGraph<N, E, R> {
    /// Create a new [`HistoryGraph`] from a set of nodes and a resolver.
    pub fn new(nodes: impl IntoIterator<Item = RelRc<N, E>>, resolver: R) -> Self {
        let mut graph = Self::with_resolver(resolver);
        for node in nodes {
            graph.insert_node(node);
        }
        graph
    }

    /// Add a `RelRc` node and all its ancestors to the `HistoryGraph`.
    ///
    /// This function is intended for users to build or extend a `HistoryGraph`
    /// by adding new nodes. It ensures that the node and its entire
    /// ancestor graph are incorporated into the history, performing
    /// deduplication as necessary.
    ///
    /// Returns the `NodeId` of the added node. If an equivalent node is already
    /// in the graph, the function will return the existing node id.
    ///
    /// # Arguments
    ///
    /// * `node` - The `RelRc` node to be added to the graph.
    ///
    /// # Returns
    ///
    /// * `NodeId` - The identifier of the added node.
    pub fn insert_node(&mut self, node: RelRc<N, E>) -> NodeId {
        let mut ancestors = FxHashMap::default();
        self.populate_ancestor_map(node, &mut ancestors)
    }

    /// Recursively populate the map of ancestor nodes until `node` can itself
    /// be added or merged with an existing node.
    ///
    /// This helper function is used internally to traverse the ancestor graph
    /// of a `RelRc` node, ensuring that all ancestor nodes are known and
    /// accounted for in the `HistoryGraph`. It computes the
    /// `AncestorGraphHash` for deduplication and determines whether the `node`
    /// can be added or merged with an existing node.
    ///
    /// # Arguments
    ///
    /// * `node` - The `RelRc` node to be processed.
    /// * `ancestors` - A mutable reference to a map of ancestor nodes and their
    ///   merge mappings.
    ///
    /// # Returns
    ///
    /// * `NodeId` - The identifier of the processed node in `self`.
    fn populate_ancestor_map(
        &mut self,
        node: RelRc<N, E>,
        ancestors: &mut FxHashMap<NodeId, Option<R::MergeMapping>>,
    ) -> NodeId {
        // Recursively add all parents
        let mut parent_ids = Vec::with_capacity(node.n_incoming());
        for p in node.all_parents().cloned() {
            let p_id = self.populate_ancestor_map(p, ancestors);
            parent_ids.push(p_id);
        }

        // Obtain key for node through Merkle hashing
        let dedup_key = {
            let incoming_edges = node.all_incoming().iter().map(|e| e.value()).collect_vec();
            let mut hasher = FxHasher64::default();
            // Combine parent hashes
            for &p_id in &parent_ids {
                hasher.write_u64(p_id.0);
            }
            // Add node hash
            self.resolver
                .dedup_key(node.value(), &incoming_edges)
                .hash(&mut hasher);

            hasher.finish()
        };

        // Find if node is equivalent to an existing node
        let candidate_equivalent_nodes = self.nodes_by_hash.get(&dedup_key);
        let existing_node = if let Some(vec) = candidate_equivalent_nodes {
            self.position_equivalent_node(&node, vec)
        } else {
            None
        };

        // Insert or merge node into graph
        if let Some((index, mapping)) = existing_node {
            // Node already in `self` => Nothing to do other than recording the mapping for
            // its children
            let node_id = NodeId(dedup_key, index);
            ancestors.insert(node_id, mapping);
            node_id
        } else {
            // Add new node to `self`
            let index = self.nodes_by_hash.get(&dedup_key).map_or(0, |v| v.len());
            let node_id = NodeId(dedup_key, index);
            // Move node parents to their equivalent nodes in `self`
            let parent_mappings = parent_ids
                .iter()
                .map(|id| ancestors[id].as_ref())
                .collect_vec();
            let new_node = self.move_parents(node, &parent_ids, &parent_mappings);

            // Add new node to `self`
            let vec = self.nodes_by_hash.entry(dedup_key).or_default();
            self.ptr_to_node_id.insert(new_node.as_ptr(), node_id);
            vec.push(new_node);

            ancestors.insert(node_id, None);
            node_id
        }
    }

    /// Find the position of a node in `candidate_equivalent_nodes` that is
    /// equivalent to `node`, along with its merge mapping if it exists.
    ///
    /// Return:
    ///  - None if no equivalent node is found.
    ///  - Some((pos, None)) if the pos-th node is exactly equal to `node`
    ///    (trivial merge mapping).
    ///  - Some((pos, Some(mapping))) if the node is equivalent to another node
    ///    and `mapping` is the merge mapping from the other node to `node`.
    fn position_equivalent_node(
        &self,
        node: &RelRc<N, E>,
        candidate_equivalent_nodes: &[RelRc<N, E>],
    ) -> Option<(usize, Option<R::MergeMapping>)> {
        let incoming_edges = node.all_incoming().iter().map(|e| e.value()).collect_vec();
        candidate_equivalent_nodes
            .iter()
            .map(|other| {
                if node.as_ptr() == other.as_ptr() {
                    return Ok(None);
                }
                let other_incoming_edges =
                    other.all_incoming().iter().map(|e| e.value()).collect_vec();
                self.resolver
                    .try_merge_mapping(
                        node.value(),
                        &incoming_edges,
                        other.value(),
                        &other_incoming_edges,
                    )
                    .map(Some)
            })
            .enumerate()
            .find_map(|(index, mapping)| mapping.ok().map(|m| (index, m)))
    }

    /// Construct a new [`RelRc`] node with the same value as `node`, but
    /// with its parents moved to `parent_ids` according to `parent_mappings`.
    ///
    /// If all parents can be left unchanged, return `node` without constructing
    /// a new [`RelRc`].
    fn move_parents(
        &self,
        node: RelRc<N, E>,
        parent_ids: &[NodeId],
        parent_mappings: &[Option<&R::MergeMapping>],
    ) -> RelRc<N, E> {
        let edge_values = node.all_incoming().iter().map(|e| e.value());
        let mut all_parents_unchanged = true;

        let mut new_parents = Vec::with_capacity(parent_ids.len());
        for (&parent_id, edge_value, mapping) in izip!(parent_ids, edge_values, parent_mappings) {
            let parent = self.get_node(parent_id);
            let new_edge_value = if let Some(mapping) = mapping {
                all_parents_unchanged = false;
                self.resolver.move_edge_source(mapping, edge_value)
            } else {
                edge_value.clone()
            };
            new_parents.push((parent.clone(), new_edge_value));
        }

        if all_parents_unchanged {
            node
        } else {
            RelRc::with_parents(node.value().clone(), new_parents)
        }
    }
}

/// A node identifier in a [`RelRcGraph`], given by a dedup key and an index.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash, From, Into)]
pub struct NodeId(pub AncestorGraphHash, pub usize);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.1 == 0 {
            write!(f, "{:x}", self.0)
        } else {
            write!(f, "{:x}#{}", self.0, self.1)
        }
    }
}

impl FromStr for NodeId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((hash_str, index_str)) = s.split_once('#') {
            let hash = u64::from_str_radix(hash_str, 16)
                .map_err(|e| format!("Invalid hash '{}': {}", hash_str, e))?;
            let index = index_str
                .parse::<usize>()
                .map_err(|e| format!("Invalid index '{}': {}", index_str, e))?;
            Ok(NodeId(hash, index))
        } else {
            let hash =
                u64::from_str_radix(s, 16).map_err(|e| format!("Invalid hash '{}': {}", s, e))?;
            Ok(NodeId(hash, 0))
        }
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
    use insta::assert_snapshot;

    use super::*;
    use crate::resolver::tests::TestResolver;
    use crate::RelRc;

    #[test]
    fn test_history_graph_merge() {
        // Create first family tree
        let grandparent1 = RelRc::new((1, 10)); // First element 1 for equivalence
        let parent1 = RelRc::with_parents((1, 20), vec![(grandparent1.clone(), 10)]);
        let child1 = RelRc::with_parents((2, 30), vec![(parent1.clone(), 20)]);

        // Create second family tree with equivalent parents but distinct child
        let grandparent2 = RelRc::new((1, 15)); // First element 1 for equivalence
        let parent2 = RelRc::with_parents((1, 25), vec![(grandparent2.clone(), 15)]);
        let child2 = RelRc::with_parents((3, 35), vec![(parent2.clone(), 25)]);
        assert_eq!(child2.incoming(0).unwrap().value(), &25);

        // Create history graph with first family
        let resolver = TestResolver;
        let mut graph = HistoryGraph::with_resolver(resolver);
        let child1_id = graph.insert_node(child1);

        // Check ancestor graph hash of child1
        let exp_hash = {
            let mut hasher = FxHasher64::default();
            hasher.write_usize(1);
            let grandparent_hash = hasher.finish();

            let mut hasher = FxHasher64::default();
            hasher.write_u64(grandparent_hash);
            hasher.write_usize(1);
            let parent_hash = hasher.finish();

            let mut hasher = FxHasher64::default();
            hasher.write_u64(parent_hash);
            hasher.write_usize(2);
            hasher.finish()
        };
        assert_eq!(child1_id, NodeId(exp_hash, 0));

        // Verify first family was added correctly
        let node_ids: Vec<_> = graph.all_node_ids().collect();
        assert_eq!(
            node_ids.len(),
            3,
            "Should have 3 nodes after adding first family"
        );

        for node_id in graph.all_node_ids() {
            for e in graph.incoming_edges(node_id) {
                dbg!(e);
                dbg!(graph.source(e));
            }
        }

        // Check parent-child relationships in graph
        let parent_id = graph.source(EdgeId {
            target: child1_id,
            index: 0,
        });
        let grandparent_id = graph.source(EdgeId {
            target: parent_id,
            index: 0,
        });
        let out_grandparent = graph
            .outgoing_edges(grandparent_id)
            .exactly_one()
            .ok()
            .unwrap();
        assert_eq!(out_grandparent.target, parent_id);
        let out_parent = graph.outgoing_edges(parent_id).exactly_one().ok().unwrap();
        assert_eq!(out_parent.target, child1_id);

        // Add second family
        let child2_id = graph.insert_node(child2);

        // Verify only one new node was added (child2)
        let node_ids: Vec<_> = graph.all_node_ids().collect();
        assert_eq!(
            node_ids.len(),
            4,
            "Should have 4 nodes after adding second family"
        );

        // Verify edge values were updated correctly
        let edge_id = graph.incoming_edges(child2_id).exactly_one().ok().unwrap();
        let child2_edge = graph.get_edge(edge_id);

        // The edge value should be updated to match the second element of the parent
        // (20)
        assert_eq!(child2_edge.value(), &20);
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
        let resolver = TestResolver;
        let mut graph = HistoryGraph::with_resolver(resolver);
        graph.insert_node(child1);
        graph.insert_node(child2);

        let serialized = graph.to_serialized();
        assert_snapshot!(serde_json::to_string_pretty(&serialized).unwrap());

        let deserialized = HistoryGraph::try_from_serialized(serialized, resolver).unwrap();

        assert_eq!(
            graph.all_node_ids().collect::<BTreeSet<_>>(),
            deserialized.all_node_ids().collect::<BTreeSet<_>>()
        );
    }
}
