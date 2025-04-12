//! Serialization and deserialization of [`RelRc`] objects.

use std::collections::BTreeMap;

use crate::{EquivalenceResolver, HistoryGraph, NodeId, RelRc};

/// A serializable representation of a [`RelRc`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedRelRc<N, E> {
    pub id: NodeId,
    pub ancestors: BTreeMap<NodeId, SerializedInnerData<N, E>>,
}

/// A serializable representation of the inner data of a [`RelRc`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedInnerData<N, E> {
    pub value: N,
    pub incoming: Vec<(NodeId, E)>,
}

impl<N: Clone, E: Clone> SerializedInnerData<N, E> {
    fn serialize_inner_data<R>(
        node_id: NodeId,
        history: &HistoryGraph<N, E, R>,
    ) -> SerializedInnerData<N, E> {
        let parent_ids = history.parents(node_id);
        let node = history.get_node(node_id);
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

impl<N: Clone, E: Clone> RelRc<N, E> {
    /// Convert a [`RelRc`] object to a serializable format.
    pub fn to_serialized(&self, resolver: impl EquivalenceResolver<N, E>) -> SerializedRelRc<N, E> {
        let mut history = HistoryGraph::with_resolver(resolver);
        let curr_id = history.insert_node(self.clone());
        let mut serialized_ancestors = BTreeMap::new();
        for node_id in history.all_node_ids() {
            let serialized = SerializedInnerData::serialize_inner_data(node_id, &history);
            serialized_ancestors.insert(node_id, serialized);
        }
        SerializedRelRc {
            id: curr_id,
            ancestors: serialized_ancestors,
        }
    }

    /// Convert a serializable representation of a [`RelRc`] object back to a
    /// [`RelRc`] object.
    pub fn from_serialized(serialized: SerializedRelRc<N, E>) -> Self {
        serialized.into()
    }
}

impl<N: Clone, E: Clone> From<SerializedRelRc<N, E>> for RelRc<N, E> {
    fn from(mut serialized: SerializedRelRc<N, E>) -> Self {
        let mut ancestors = BTreeMap::new();
        serialized.deserialize_node(serialized.id, &mut ancestors)
    }
}

impl<N, E> SerializedRelRc<N, E> {
    fn deserialize_node(
        &mut self,
        node_id: NodeId,
        ancestors: &mut BTreeMap<NodeId, RelRc<N, E>>,
    ) -> RelRc<N, E> {
        let node_serialized = self.ancestors.remove(&node_id).expect("invalid node_id");

        // Recursively deserialize ancestors
        for (parent_id, _) in node_serialized.incoming.iter() {
            if !ancestors.contains_key(parent_id) {
                let ancestor = self.deserialize_node(*parent_id, ancestors);
                ancestors.insert(*parent_id, ancestor);
            }
        }

        // Create incoming edges
        let incoming = node_serialized
            .incoming
            .into_iter()
            .map(|(parent_id, edge_value)| {
                let parent = ancestors[&parent_id].clone();
                (parent, edge_value)
            });

        RelRc::with_parents(node_serialized.value, incoming)
    }
}
