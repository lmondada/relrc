use derive_where::derive_where;
use petgraph::visit;

use crate::edge::InnerEdgeData;
use crate::{HistoryGraph, NodeId};

use crate::history::EdgeId;

/// An edge reference in an [`AncestorGraph`].
///
/// At construction time it must be guaranteed that the edge will exist
/// for the lifetime `'a`.
#[derive(Debug)]
#[derive_where(Clone, Copy)]
pub struct EdgeRef<'a, N, E> {
    id: EdgeId,
    history: &'a HistoryGraph<N, E>,
}

impl<'a, N, E> EdgeRef<'a, N, E> {
    pub(super) fn new(id: EdgeId, history: &'a HistoryGraph<N, E>) -> Self {
        Self { id, history }
    }

    fn edge_data(&self) -> &'a InnerEdgeData<N, E>
    where
        N: 'a,
    {
        self.history.get_edge(self.id).expect("edge is valid")
    }
}

impl<'a, N, E> visit::EdgeRef for EdgeRef<'a, N, E> {
    type NodeId = NodeId;

    type EdgeId = EdgeId;

    type Weight = E;

    fn source(&self) -> Self::NodeId {
        self.history.source(self.id).expect("edge is valid")
    }

    fn target(&self) -> Self::NodeId {
        self.history.target(self.id).expect("edge is valid")
    }

    fn weight(&self) -> &Self::Weight {
        self.edge_data().value()
    }

    fn id(&self) -> Self::EdgeId {
        self.id
    }
}
