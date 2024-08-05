//! Implementation of the [`petgraph`] graph traits

mod edge_ref;
pub use edge_ref::EdgeRef;

use petgraph::{
    visit::{
        Data, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoEdgesDirected, IntoNeighbors,
        IntoNeighborsDirected,
    },
    Direction,
};

use crate::{EdgeId, GraphView, NodeId};

impl<'a, N, E> GraphBase for &'a GraphView<N, E> {
    type EdgeId = EdgeId<N, E>;
    type NodeId = NodeId<N, E>;
}

impl<'a, N, E> GraphRef for &'a GraphView<N, E> {}

impl<'a, N, E> IntoNeighbors for &'a GraphView<N, E> {
    type Neighbors = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors(self, n: Self::NodeId) -> Self::Neighbors {
        self.neighbors_directed(n, Direction::Outgoing)
    }
}

impl<'a, N, E> IntoNeighborsDirected for &'a GraphView<N, E> {
    type NeighborsDirected = Box<dyn Iterator<Item = Self::NodeId> + 'a>;

    fn neighbors_directed(self, node_ptr: Self::NodeId, d: Direction) -> Self::NeighborsDirected {
        let node = self.get_node(node_ptr);
        match d {
            Direction::Outgoing => Box::new(node.all_children().map(|c| (&c).into())),
            Direction::Incoming => Box::new(node.all_parents().map(|c| c.into())),
        }
    }
}

impl<'a, N, E> Data for &'a GraphView<N, E> {
    type NodeWeight = N;
    type EdgeWeight = E;
}

impl<'a, N, E> IntoEdgeReferences for &'a GraphView<N, E> {
    type EdgeRef = EdgeRef<'a, N, E>;

    type EdgeReferences = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edge_references(self) -> Self::EdgeReferences {
        Box::new(self.all_nodes().iter().flat_map(|&node_id| {
            let node = self.get_node(node_id);
            (0..node.n_incoming()).map(move |i| unsafe { EdgeRef::new_unchecked(node_id, i) })
        }))
    }
}

impl<'a, N, E> IntoEdges for &'a GraphView<N, E> {
    type Edges = Box<dyn Iterator<Item = Self::EdgeRef> + 'a>;

    fn edges(self, node_id: Self::NodeId) -> Self::Edges {
        let node = self.get_node(node_id);
        let edges = node.all_outgoing_weak().to_vec();
        Box::new(
            edges
                .into_iter()
                .map(|e| unsafe { EdgeRef::from_weak_unchecked(e) }),
        )
    }
}

impl<'a, N, E> IntoEdgesDirected for &'a GraphView<N, E> {
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
