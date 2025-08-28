//! Serialization and deserialization of [`RelRc`] objects.

use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use derive_more::{From, Into};
use fxhash::FxHashSet;
use itertools::Itertools;
use slotmap_fork_lmondada::{SecondaryMap, SlotMap};

use crate::{HistoryGraph, NodeId, Registry, RelRc};

/// A serializable representation of a [`RelRc`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedRelRc<N, E> {
    /// The ID of the current node.
    pub id: NodeId,
    /// The ancestors of the current node.
    pub ancestors_graph: SerializedHistoryGraph<N, E>,
}

/// A serializable representation of a [`HistoryGraph`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedHistoryGraph<N, E> {
    /// The node IDs of the graph.
    pub nodes: BTreeSet<NodeId>,
    /// All nodes required to reconstruct the graph (i.e. the nodes
    /// in `nodes` and their ancestors).
    pub registry: SerializedRegistry<N, E>,
}

/// A serializable representation of the inner data of a [`RelRc`] object.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedInnerData<N, E> {
    /// The value of the node.
    pub value: N,
    /// The incoming edges of the node.
    pub incoming: Vec<(NodeId, E)>,
}

impl<N, E> SerializedInnerData<N, E> {
    /// Map the value of the node.
    pub fn map_value<M>(self, f: impl FnOnce(N) -> M) -> SerializedInnerData<M, E> {
        SerializedInnerData {
            value: f(self.value),
            incoming: self.incoming,
        }
    }
}

impl<N: Clone, E: Clone> SerializedInnerData<N, E> {
    fn serialize_inner_data(
        node_id: NodeId,
        registry: &Registry<N, E>,
    ) -> SerializedInnerData<N, E> {
        let node = registry.get(node_id).expect("valid node");
        let parent_ids = node
            .all_parents()
            .map(|n| registry.get_id(n).expect("valid node"));
        let value = node.value().clone();
        let incoming = parent_ids
            .zip(node.all_incoming())
            .map(|(parent_id, edge)| {
                let edge_value = edge.value().clone();
                (parent_id, edge_value)
            })
            .collect();
        Self { value, incoming }
    }
}

/// A serializable representation of a [`Registry`] object.
///
/// You typically do not want to use this type directly, as registries shouldn't
/// be serialised on their own. (They only keep weak references to nodes).
#[derive(Debug, Clone, From, Into)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedRegistry<N, E> {
    /// The nodes in the registry and their serialized data.
    pub nodes: SlotMap<NodeId, SerializedInnerData<N, E>>,
}

impl<N: Clone, E: Clone> SerializedRegistry<N, E> {
    /// Map the value of the nodes in the registry.
    pub fn map_nodes<M>(&self, mut f: impl FnMut(N) -> M) -> SerializedRegistry<M, E> {
        SerializedRegistry {
            nodes: self.nodes.map(|_, v| v.clone().map_value(&mut f)),
        }
    }
}

impl<N, E> Registry<N, E> {
    /// Convert a [`Registry`] object to its serializable format.
    pub fn to_serialized(&self) -> SerializedRegistry<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let nodes = self
            .as_slotmap()
            .map(|k, _| SerializedInnerData::serialize_inner_data(k, self));
        SerializedRegistry { nodes }
    }

    /// Convert a serializable representation of a [`Registry`] object back to a
    /// [`Registry`] object.
    ///
    /// Return the deserialised [`Registry`] object alongside the deserialised
    /// nodes. The registry only contains weak references to the nodes.
    pub fn from_serialized(
        serialized: SerializedRegistry<N, E>,
    ) -> (Self, SecondaryMap<NodeId, RelRc<N, E>>) {
        let nodes_set = serialized.nodes.map(|_, _| ());
        let nodes = deserialize_nodes(serialized.nodes);
        let registry = nodes_set.map(|k, ()| nodes.get(k).expect("invalid node_id").downgrade());

        (Self::from_slotmap(registry), nodes)
    }
}

impl<N, E> RelRc<N, E> {
    /// Convert a [`RelRc`] object to a serializable format.
    ///
    /// Requires a [`Registry`] to identify serialized nodes using IDs.
    pub fn to_serialized(
        &self,
        registry: impl Into<Rc<RefCell<Registry<N, E>>>>,
    ) -> SerializedRelRc<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let mut history = HistoryGraph::with_registry(registry);
        let curr_id = history.insert_ancestors(self.clone());
        SerializedRelRc {
            id: curr_id,
            ancestors_graph: history.to_serialized(),
        }
    }

    /// Convert a serializable representation of a [`RelRc`] object back to a
    /// [`RelRc`] object.
    pub fn from_serialized(serialized: SerializedRelRc<N, E>) -> Self {
        let history = HistoryGraph::from_serialized(serialized.ancestors_graph);
        history.get_node(serialized.id).expect("valid node").clone()
    }
}

impl<N, E> HistoryGraph<N, E> {
    /// Convert a [`HistoryGraph`] object to its serializable format.
    pub fn to_serialized(&self) -> SerializedHistoryGraph<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let nodes = BTreeSet::from_iter(self.all_node_ids());
        let mut registry = self.registry().borrow().clone();
        let ancestors = nodes
            .iter()
            .flat_map(|&n| {
                let node = registry.get(n).expect("invalid node");
                node.all_ancestors()
                    .map(|n| registry.get_id_or_insert(n))
                    .collect_vec()
            })
            .collect::<FxHashSet<_>>();

        let mut ser_registry = registry.to_serialized();
        ser_registry.nodes.retain(|k, _| ancestors.contains(&k));

        SerializedHistoryGraph {
            nodes,
            registry: ser_registry,
        }
    }

    /// Convert a serializable representation of a [`HistoryGraph`] object back
    /// to a [`HistoryGraph`] object.
    pub fn from_serialized(serialized: SerializedHistoryGraph<N, E>) -> Self {
        let (registry, mut all_nodes) = Registry::from_serialized(serialized.registry);
        let keep_nodes = FxHashSet::from_iter(serialized.nodes.iter().copied());
        assert!(keep_nodes.iter().all(|&k| all_nodes.contains_key(k)));
        assert!(keep_nodes.iter().all(|&k| registry.contains_id(k)));
        all_nodes.retain(|k, _| keep_nodes.contains(&k));
        assert!(all_nodes.values().all(|v| registry.contains(v)));

        HistoryGraph::new(all_nodes.into_iter().map(|(_, n)| n), registry)
    }
}

impl<N, E> From<SerializedRelRc<N, E>> for RelRc<N, E> {
    fn from(serialized: SerializedRelRc<N, E>) -> Self {
        Self::from_serialized(serialized)
    }
}

impl<N: Clone, E: Clone> From<HistoryGraph<N, E>> for SerializedHistoryGraph<N, E> {
    fn from(value: HistoryGraph<N, E>) -> Self {
        value.to_serialized()
    }
}

impl<N, E> From<SerializedHistoryGraph<N, E>> for HistoryGraph<N, E> {
    fn from(value: SerializedHistoryGraph<N, E>) -> Self {
        HistoryGraph::from_serialized(value)
    }
}

/// Add `node_id` and its ancestors to the registry.
fn deserialize_nodes<N, E>(
    mut serialised_nodes: SlotMap<NodeId, SerializedInnerData<N, E>>,
) -> SecondaryMap<NodeId, RelRc<N, E>> {
    let mut all_nodes = SecondaryMap::new();

    fn deserialize_node<N, E>(
        node_id: NodeId,
        serialised_nodes: &mut SlotMap<NodeId, SerializedInnerData<N, E>>,
        all_nodes: &mut SecondaryMap<NodeId, RelRc<N, E>>,
    ) {
        let node_ser = serialised_nodes.remove(node_id).expect("invalid node_id");

        // Recursively deserialize ancestors
        for &(parent_id, _) in node_ser.incoming.iter() {
            if !all_nodes.contains_key(parent_id) {
                deserialize_node(parent_id, serialised_nodes, all_nodes);
            }
        }

        // Create incoming edges
        let incoming = node_ser
            .incoming
            .into_iter()
            .map(|(parent_id, edge_value)| {
                let parent = all_nodes.get(parent_id).expect("valid dfs order");
                (parent.clone(), edge_value)
            });

        let node_deser = RelRc::with_parents(node_ser.value, incoming);
        all_nodes.insert(node_id, node_deser);
    }

    while let Some((node_id, _)) = serialised_nodes.iter().next() {
        deserialize_node(node_id, &mut serialised_nodes, &mut all_nodes);
    }

    all_nodes
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod tests {
    use super::*;
    use crate::{HistoryGraph, RelRc};
    use insta::assert_yaml_snapshot;

    #[test]
    fn test_relrc_serialization() {
        // Create a simple RelRc node
        let node = RelRc::<_, ()>::new(42);
        let serialized = node.to_serialized(Registry::new());

        assert_yaml_snapshot!(serialized);

        let deser = RelRc::from_serialized(serialized);
        assert_eq!(deser.value(), &42);
        assert_eq!(deser.all_parents().count(), 0);
        assert_eq!(deser.all_ancestors().count(), 1);
    }

    #[test]
    fn test_relrc_with_parents_serialization() {
        // Create a chain of nodes: parent -> child -> grandchild
        let parent = RelRc::new("parent");
        let child = RelRc::with_parents("child", [(parent, "edge_to_child")]);
        let grandchild = RelRc::with_parents("grandchild", [(child, "edge_to_grandchild")]);

        let serialized = grandchild.to_serialized(Registry::new());

        assert_yaml_snapshot!(serialized);

        let deser = RelRc::from_serialized(serialized);
        assert_eq!(deser.value(), &"grandchild");
        assert_eq!(
            deser.all_parents().map(|n| n.value()).collect_vec(),
            vec![&"child"]
        );
        assert_eq!(deser.all_ancestors().count(), 3);
    }

    #[test]
    fn test_relrc_with_multiple_parents_serialization() {
        // Create a diamond pattern: parent1, parent2 -> child
        let parent1 = RelRc::new("parent1");
        let parent2 = RelRc::new("parent2");
        let child = RelRc::with_parents(
            "child",
            [
                (parent1.clone(), "edge_from_parent1"),
                (parent2.clone(), "edge_from_parent2"),
            ],
        );
        let sibling = RelRc::with_parents("sibling", [(parent1.clone(), "edge_from_parent1")]);

        let registry = Registry::from_iter([&parent1, &parent2, &sibling, &child]);

        let serialized = child.to_serialized(registry);

        assert_eq!(serialized.ancestors_graph.nodes.len(), 3);

        assert_yaml_snapshot!(serialized);

        let deser = RelRc::from_serialized(serialized);
        assert_eq!(deser.value(), &"child");
        assert_eq!(
            deser.all_parents().map(|n| n.value()).collect_vec(),
            vec![&"parent1", &"parent2"]
        );
        assert_eq!(deser.all_children().len(), 0);
        assert_eq!(deser.all_ancestors().count(), 3);
    }

    #[test]
    fn test_history_graph_serialization() {
        // Create a graph with multiple nodes
        let root1 = RelRc::new("root1");
        let root2 = RelRc::new("root2");
        let child1 = RelRc::with_parents("child1", [(root1.clone(), "edge1")]);
        let child2 = RelRc::with_parents(
            "child2",
            [(root1.clone(), "edge2"), (root2.clone(), "edge3")],
        );

        let graph = HistoryGraph::from_nodes([child1.clone(), child2.clone()]);
        let serialized = SerializedHistoryGraph::from(graph.clone());

        // Test JSON serialization
        assert_yaml_snapshot!("two_parents_two_children", serialized);

        let deser = HistoryGraph::from_serialized(serialized);

        dbg!(&deser
            .all_node_ids()
            .map(|n| deser.get_node(n).unwrap())
            .collect_vec());
        dbg!(&deser.registry().borrow());
        assert_eq!(
            deser.all_node_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from_iter([
                deser.registry().borrow().get_id(&child1).unwrap(),
                deser.registry().borrow().get_id(&child2).unwrap(),
            ])
        );
        assert_eq!(deser.registry().borrow().len(), 4);

        let graph = HistoryGraph::from_nodes([child1.clone()]);
        let serialized = SerializedHistoryGraph::from(graph.clone());

        assert_yaml_snapshot!("one_parent_one_child", serialized);

        let deser = HistoryGraph::from_serialized(serialized);
        assert_eq!(
            deser.all_node_ids().collect::<BTreeSet<_>>(),
            BTreeSet::from_iter([deser.registry().borrow().get_id(&child1).unwrap(),])
        );
        assert_eq!(deser.registry().borrow().len(), 2);
    }
}
