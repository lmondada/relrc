use std::marker::PhantomData;

use petgraph::visit;

use crate::{edge::InnerEdgeData, RelRc, RelWeak, WeakEdge};

use crate::ancestor_graph::{EdgeId, NodeId};

/// An edge reference in an [`AncestorGraph`].
///
/// At construction time it must be guaranteed that the edge will exist
/// for the lifetime `'a`.
pub struct EdgeRef<'a, N, E> {
    target: NodeId<N, E>,
    index: usize,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a, N, E> EdgeRef<'a, N, E> {
    fn edge_data(&self) -> &'a InnerEdgeData<N, E>
    where
        N: 'a,
    {
        let target = unsafe { &*self.target.0 };
        target.incoming(self.index).unwrap()
    }

    pub(super) unsafe fn new_unchecked(target: NodeId<N, E>, index: usize) -> Self {
        Self {
            target,
            index,
            _lifetime: PhantomData,
        }
    }

    pub(super) unsafe fn from_weak_unchecked(edge: WeakEdge<N, E>) -> Self {
        let index = edge.index;
        let target = NodeId(RelWeak::as_ptr(&edge.target));
        Self {
            target,
            index,
            _lifetime: PhantomData,
        }
    }
}

impl<'a, N, E> Clone for EdgeRef<'a, N, E> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, N, E> Copy for EdgeRef<'a, N, E> {}

impl<'a, N, E> visit::EdgeRef for EdgeRef<'a, N, E> {
    type NodeId = NodeId<N, E>;

    type EdgeId = EdgeId<N, E>;

    type Weight = E;

    fn source(&self) -> Self::NodeId {
        RelRc::as_ptr(self.edge_data().source()).into()
    }

    fn target(&self) -> Self::NodeId {
        self.target
    }

    fn weight(&self) -> &Self::Weight {
        self.edge_data().value()
    }

    fn id(&self) -> Self::EdgeId {
        EdgeId {
            target: self.target,
            index: self.index,
        }
    }
}
