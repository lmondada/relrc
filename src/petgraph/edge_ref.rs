use derive_where::derive_where;
use petgraph::visit;

use crate::edge::InnerEdgeData;
use crate::HistoryGraph;

use crate::history::{EdgeId, NodeId};

/// An edge reference in an [`AncestorGraph`].
///
/// At construction time it must be guaranteed that the edge will exist
/// for the lifetime `'a`.
#[derive(Debug)]
#[derive_where(Clone, Copy)]
pub struct EdgeRef<'a, N, E, R> {
    id: EdgeId,
    history: &'a HistoryGraph<N, E, R>,
}

impl<'a, N, E, R> EdgeRef<'a, N, E, R> {
    pub(super) fn new(id: EdgeId, history: &'a HistoryGraph<N, E, R>) -> Self {
        Self { id, history }
    }

    fn edge_data(&self) -> &'a InnerEdgeData<N, E>
    where
        N: 'a,
    {
        self.history.get_edge(self.id)
    }
}

impl<'a, N, E, R> visit::EdgeRef for EdgeRef<'a, N, E, R> {
    type NodeId = NodeId;

    type EdgeId = EdgeId;

    type Weight = E;

    fn source(&self) -> Self::NodeId {
        self.history.source(self.id)
    }

    fn target(&self) -> Self::NodeId {
        self.history.target(self.id)
    }

    fn weight(&self) -> &Self::Weight {
        self.edge_data().value()
    }

    fn id(&self) -> Self::EdgeId {
        self.id
    }
}
