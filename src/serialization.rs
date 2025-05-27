//! Serialization and deserialization of [`RelRc`] objects.

use std::collections::BTreeMap;
#[cfg(feature = "serde")]
use std::str::FromStr;

use crate::{
    resolver::{InvalidResolver, ResolverId},
    EquivalenceResolver, HistoryGraph, NodeId, RelRc,
};

/// A serializable representation of a [`RelRc`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedRelRc<N, E> {
    /// The ID of the current node.
    pub id: NodeId,
    /// The ancestors of the current node.
    pub ancestors: BTreeMap<NodeId, SerializedInnerData<N, E>>,
}

/// A serializable representation of a [`HistoryGraph`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(bound(serialize = "N: serde::Serialize, E: serde::Serialize"))
)]
#[cfg_attr(
    feature = "serde",
    serde(bound(deserialize = "N: serde::de::DeserializeOwned, E: serde::de::DeserializeOwned"))
)]
pub struct SerializedHistoryGraph<N, E, R> {
    /// The nodes of the graph.
    pub nodes: BTreeMap<NodeId, SerializedInnerData<N, E>>,
    /// The ID of the resolver.
    pub resolver_id: ResolverId<N, E, R>,
}

/// A serializable representation of the inner data of a [`RelRc`] object.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SerializedInnerData<N, E> {
    /// The value of the node.
    pub value: N,
    /// The incoming edges of the node.
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
    pub fn from_serialized(mut serialized: SerializedRelRc<N, E>) -> Self {
        let mut ancestors_deser = BTreeMap::new();
        deserialize_node(
            serialized.id,
            &mut serialized.ancestors,
            &mut ancestors_deser,
        )
        .clone()
    }
}

impl<N: Clone, E: Clone, R: EquivalenceResolver<N, E>> HistoryGraph<N, E, R> {
    /// Convert a [`HistoryGraph`] object to its serializable format.
    pub fn to_serialized(&self) -> SerializedHistoryGraph<N, E, R> {
        let mut nodes = BTreeMap::new();
        for node_id in self.all_node_ids() {
            let serialized = SerializedInnerData::serialize_inner_data(node_id, self);
            nodes.insert(node_id, serialized);
        }
        SerializedHistoryGraph {
            nodes,
            resolver_id: self.resolver_id().into(),
        }
    }

    /// Convert a serializable representation of a [`RelRc`] object back to a
    /// [`RelRc`] object.
    pub fn try_from_serialized(
        mut serialized: SerializedHistoryGraph<N, E, R>,
        resolver: R,
    ) -> Result<Self, InvalidResolver> {
        if ResolverId::from(&resolver) != serialized.resolver_id {
            return Err(InvalidResolver(
                resolver.id(),
                serialized.resolver_id.into(),
            ));
        }
        let mut ancestors_deser = BTreeMap::new();
        while let Some(&node_id) = serialized.nodes.keys().next() {
            deserialize_node(node_id, &mut serialized.nodes, &mut ancestors_deser);
        }
        Ok(HistoryGraph::new(ancestors_deser.into_values(), resolver))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NodeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for NodeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        NodeId::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl<N: Clone, E: Clone> From<SerializedRelRc<N, E>> for RelRc<N, E> {
    fn from(serialized: SerializedRelRc<N, E>) -> Self {
        Self::from_serialized(serialized)
    }
}

impl<'g, N: Clone, E: Clone, R: EquivalenceResolver<N, E>> From<&'g HistoryGraph<N, E, R>>
    for SerializedHistoryGraph<N, E, R>
{
    fn from(value: &'g HistoryGraph<N, E, R>) -> Self {
        value.to_serialized()
    }
}

fn deserialize_node<'a, N: Clone, E: Clone>(
    node_id: NodeId,
    ancestors_ser: &mut BTreeMap<NodeId, SerializedInnerData<N, E>>,
    ancestors_deser: &'a mut BTreeMap<NodeId, RelRc<N, E>>,
) -> &'a RelRc<N, E> {
    let node_ser = ancestors_ser.remove(&node_id).expect("invalid node_id");

    // Recursively deserialize ancestors
    for (parent_id, _) in node_ser.incoming.iter() {
        if !ancestors_deser.contains_key(parent_id) {
            deserialize_node(*parent_id, ancestors_ser, ancestors_deser);
        }
    }

    // Create incoming edges
    let incoming = node_ser
        .incoming
        .into_iter()
        .map(|(parent_id, edge_value)| {
            let parent = ancestors_deser[&parent_id].clone();
            (parent, edge_value)
        });

    let node_deser = RelRc::with_parents(node_ser.value, incoming);
    ancestors_deser.insert(node_id, node_deser.clone());
    &ancestors_deser[&node_id]
}
