//! Implementation of the [`petgraph`] graph traits

mod edge_ref;
use std::collections::HashSet;

pub use edge_ref::EdgeRef;

use petgraph::{
    visit::{
        Data, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoEdgesDirected, IntoNeighbors,
        IntoNeighborsDirected, IntoNodeIdentifiers, Visitable,
    },
    Direction,
};

use crate::{EdgeId, HistoryGraph, NodeId};

impl<'a, N, E, R> GraphBase for &'a HistoryGraph<N, E, R> {
    type EdgeId = EdgeId;
    type NodeId = NodeId;
}

impl<'a, N, E, R> GraphRef for &'a HistoryGraph<N, E, R> {}

impl<'a, N, E, R> IntoNeighbors for &'a HistoryGraph<N, E, R> {
    type Neighbors = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors(self, n: Self::NodeId) -> Self::Neighbors {
        self.neighbors_directed(n, Direction::Outgoing)
    }
}

impl<'a, N, E, R> IntoNeighborsDirected for &'a HistoryGraph<N, E, R> {
    type NeighborsDirected = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors_directed(self, node_id: Self::NodeId, d: Direction) -> Self::NeighborsDirected {
        match d {
            Direction::Outgoing => Box::new(self.outgoing_edges(node_id).map(|e| e.target)),
            Direction::Incoming => Box::new(self.incoming_edges(node_id).map(|e| self.source(e))),
        }
    }
}

impl<'a, N, E, R> Data for &'a HistoryGraph<N, E, R> {
    type NodeWeight = N;
    type EdgeWeight = E;
}

impl<'a, N, E, R> IntoEdgeReferences for &'a HistoryGraph<N, E, R> {
    type EdgeRef = EdgeRef<'a, N, E, R>;

    type EdgeReferences = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edge_references(self) -> Self::EdgeReferences {
        Box::new(self.all_node_ids().flat_map(move |node_id| {
            self.incoming_edges(node_id)
                .map(move |edge_id| EdgeRef::new(edge_id, self))
        }))
    }
}

impl<'a, N, E, R> IntoNodeIdentifiers for &'a HistoryGraph<N, E, R> {
    type NodeIdentifiers = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn node_identifiers(self) -> Self::NodeIdentifiers {
        Box::new(self.all_node_ids())
    }
}

impl<'a, N, E, R> IntoEdges for &'a HistoryGraph<N, E, R> {
    type Edges = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edges(self, node_id: Self::NodeId) -> Self::Edges {
        Box::new(
            self.outgoing_edges(node_id)
                .map(|edge_id| EdgeRef::new(edge_id, self)),
        )
    }
}

impl<'a, N, E, R> IntoEdgesDirected for &'a HistoryGraph<N, E, R> {
    type EdgesDirected = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edges_directed(self, node_id: Self::NodeId, d: Direction) -> Self::EdgesDirected {
        match d {
            Direction::Outgoing => Box::new(
                self.outgoing_edges(node_id)
                    .map(|edge_id| EdgeRef::new(edge_id, self)),
            ),
            Direction::Incoming => Box::new(
                self.incoming_edges(node_id)
                    .map(|edge_id| EdgeRef::new(edge_id, self)),
            ),
        }
    }
}

impl<'a, N, E, R> Visitable for &'a HistoryGraph<N, E, R> {
    type Map = HashSet<NodeId>;

    #[doc = r" Create a new visitor map"]
    fn visit_map(&self) -> Self::Map {
        HashSet::new()
    }

    #[doc = r" Reset the visitor map (and resize to new size of graph if needed)"]
    fn reset_map(&self, map: &mut Self::Map) {
        map.clear();
    }
}
