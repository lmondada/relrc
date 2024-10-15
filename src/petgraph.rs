//! Implementation of the [`petgraph`] graph traits

mod edge_ref;
use std::collections::HashSet;
use std::hash::Hash;

pub use edge_ref::EdgeRef;

use petgraph::{
    visit::{
        Data, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoEdgesDirected, IntoNeighbors,
        IntoNeighborsDirected, IntoNodeIdentifiers, Visitable,
    },
    Direction,
};

use crate::{EdgeId, NodeId, RelRcGraph};

impl<'a, N, E> GraphBase for &'a RelRcGraph<N, E> {
    type EdgeId = EdgeId<N, E>;
    type NodeId = NodeId<N, E>;
}

impl<'a, N, E> GraphRef for &'a RelRcGraph<N, E> {}

impl<'a, N: Hash, E: Hash> IntoNeighbors for &'a RelRcGraph<N, E> {
    type Neighbors = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors(self, n: Self::NodeId) -> Self::Neighbors {
        self.neighbors_directed(n, Direction::Outgoing)
    }
}

impl<'a, N: Hash, E: Hash> IntoNeighborsDirected for &'a RelRcGraph<N, E> {
    type NeighborsDirected = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors_directed(self, node_ptr: Self::NodeId, d: Direction) -> Self::NeighborsDirected {
        let node = self.get_node(node_ptr);
        match d {
            Direction::Outgoing => Box::new(node.all_children().map(|c| (&c).into())),
            Direction::Incoming => Box::new(node.all_parents().map(|c| c.into())),
        }
    }
}

impl<'a, N, E> Data for &'a RelRcGraph<N, E> {
    type NodeWeight = N;
    type EdgeWeight = E;
}

impl<'a, N: Hash, E: Hash> IntoEdgeReferences for &'a RelRcGraph<N, E> {
    type EdgeRef = EdgeRef<'a, N, E>;

    type EdgeReferences = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edge_references(self) -> Self::EdgeReferences {
        Box::new(self.all_nodes().iter().flat_map(|&node_id| {
            let node = self.get_node(node_id);
            (0..node.n_incoming()).map(move |i| unsafe { EdgeRef::new_unchecked(node_id, i) })
        }))
    }
}

impl<'a, N: Hash, E: Hash> IntoNodeIdentifiers for &'a RelRcGraph<N, E> {
    type NodeIdentifiers = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn node_identifiers(self) -> Self::NodeIdentifiers {
        Box::new(self.all_nodes().iter().copied())
    }
}

impl<'a, N: Hash, E: Hash> IntoEdges for &'a RelRcGraph<N, E> {
    type Edges = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edges(self, node_id: Self::NodeId) -> Self::Edges {
        Box::new(self.outgoing_edges(node_id).map(|e| e.into()))
    }
}

impl<'a, N: Hash, E: Hash> IntoEdgesDirected for &'a RelRcGraph<N, E> {
    type EdgesDirected = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edges_directed(self, node_id: Self::NodeId, d: Direction) -> Self::EdgesDirected {
        let node = self.get_node(node_id);
        match d {
            Direction::Outgoing => Box::new(
                node.all_outgoing_weak()
                    .to_vec()
                    .into_iter()
                    .map(|e| unsafe { EdgeRef::from_weak_unchecked(e.clone()) }),
            ),
            Direction::Incoming => Box::new(
                (0..node.n_incoming()).map(move |i| unsafe { EdgeRef::new_unchecked(node_id, i) }),
            ),
        }
    }
}

impl<'a, N, E> Visitable for &'a RelRcGraph<N, E> {
    type Map = HashSet<NodeId<N, E>>;

    #[doc = r" Create a new visitor map"]
    fn visit_map(self: &Self) -> Self::Map {
        HashSet::new()
    }

    #[doc = r" Reset the visitor map (and resize to new size of graph if needed)"]
    fn reset_map(self: &Self, map: &mut Self::Map) {
        map.clear();
    }
}
