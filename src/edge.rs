//! Parent-child relationships between [`RelRc`] objects.

use std::hash::Hash;
use std::ops::Deref;

use derive_where::derive_where;

use crate::{RelRc, RelWeak};

/// A parent-child relationship between two [`RelRc`] objects.
///
/// Stored and owned by the child object. Unlike [`InnerData`], this is not
/// wrapped in a `Rc`, but rather relies on the counted reference of the edge
/// target.
///
/// Note: the implementation assumes that the edge target is always the owner of
/// the [`InnerEdgeData`]. Calls to [`InnerEdgeData::target`] may otherwise panic.
/// This also means that [`InnerEdgeData`] cannot be cloned.
#[derive(Debug)]
#[derive_where(Clone, Hash; E)]
pub struct InnerEdgeData<N, E> {
    /// The value of the edge.
    pub(crate) value: E,
    /// The source (parent) of the edge (strong reference).
    pub(crate) source: RelRc<N, E>,
    /// The target (child) of the edge.
    ///
    /// This is a weak reference to avoid reference loops between the edge and
    /// the target node. However, the target is always the owner of the edge,
    /// so this reference can always be upgraded.
    pub(crate) target: RelWeak<N, E>,
}

impl<N, E> InnerEdgeData<N, E> {
    pub(crate) fn new(value: E, source: RelRc<N, E>, target: RelWeak<N, E>) -> Self {
        Self {
            value,
            source,
            target,
        }
    }

    /// The value of the edge.
    pub fn value(&self) -> &E {
        &self.value
    }

    /// The source node of the edge.
    pub fn source(&self) -> &RelRc<N, E> {
        &self.source
    }

    /// Downgrade the edge to a [`WeakEdge`].
    ///
    /// Requires the position of the edge in the target's incoming edges.
    pub(crate) fn downgrade(&self, target_pos: usize) -> WeakEdge<N, E> {
        WeakEdge::new(target_pos, self.target.clone())
    }
}

impl<N: Hash, E: Hash> InnerEdgeData<N, E> {
    /// The target node of the edge.
    ///
    /// This upgrades the target node and returns a strong reference. It panics
    /// if the target node is no longer alive.
    pub fn target(&self) -> RelRc<N, E> {
        self.target
            .upgrade()
            .expect("target node is no longer alive")
    }
}

/// Weak reference to an edge.
///
/// If the reference is still valid, upgrades to a [`StrongEdge`]---which can be
/// treated as a strong reference to the edge.
///
/// This is implemented by keeping a weak reference to the target node which
/// owns this edge. If the owner node can be upgraded, then we can recover the
/// [`Edge`] object by looking up the index of this edge in the owner's incoming
/// edges.
///
/// Upgrades to [`Edge`] if the reference is valid.
#[derive(Debug)]
#[derive_where(Clone, Hash)]
pub struct WeakEdge<N, E> {
    /// The index of the edge in the owner node's incoming edges.
    pub(crate) index: usize,
    /// The target node (and owner) of the edge.
    pub(crate) target: RelWeak<N, E>,
}

impl<N, E> WeakEdge<N, E> {
    pub(crate) fn new(index: usize, target: RelWeak<N, E>) -> Self {
        Self { index, target }
    }

    /// Check if two weak references point to the same underlying data
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.index == other.index && RelWeak::ptr_eq(&self.target, &other.target)
    }
    /// The edge target as a weak reference.
    pub fn target(&self) -> &RelWeak<N, E> {
        &self.target
    }
}

/// Strong reference to an edge.
///
/// Will keep the edge and the target object of the edge alive for as long as this
/// reference is in scope.
#[derive(Debug)]
#[derive_where(Clone, Hash)]
pub struct Edge<N, E> {
    /// The index of the edge in the owner node's incoming edges.
    index: usize,
    /// The target node (and owner) of the edge.
    target: RelRc<N, E>,
}

impl<N, E> Edge<N, E> {
    /// The target node of the edge.
    ///
    /// This is equivalent to derefencing `self` into a [`EdgeData`] and calling
    /// [`EdgeData::target`], but avoids creating a new reference.
    pub fn target(&self) -> &RelRc<N, E> {
        &self.target
    }

    /// Consume the edge and return the target object.
    pub fn into_target(self) -> RelRc<N, E> {
        self.target
    }
}

impl<N, E> Deref for Edge<N, E> {
    type Target = InnerEdgeData<N, E>;

    fn deref(&self) -> &Self::Target {
        self.target.incoming(self.index).unwrap()
    }
}

impl<N: Hash, E: Hash> WeakEdge<N, E> {
    /// Upgrades to a [`Edge`] if the reference is still valid.
    pub fn upgrade(&self) -> Option<Edge<N, E>> {
        let target = self.target.upgrade()?;
        Some(Edge {
            index: self.index,
            target,
        })
    }
}
