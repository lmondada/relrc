use std::{
    cell::RefCell,
    ops::Deref,
    rc::{Rc, Weak},
};

use derive_more::From;

use crate::{edge::InnerEdgeData, Edge, WeakEdge};

/// A single-threaded reference-counted pointer, optionally with relationships
/// to other [`RelRc`] objects.
///
/// A new [`RelRc`] object is created with either
///  - [`RelRc::new`]: behaves identically to [`Rc::new`], or
///  - [`RelRc::with_parents`]: creates a new [`RelRc`] object, with a list of
///    parent [`RelRc`] objects.
///
/// A [`RelRc`] object will remain in memory for as long as there is at least
/// one strong reference to it or to one of its descendants.
///
/// ## Immutability
/// Just like [`Rc`], [`RelRc`] objects are immutable. Once a [`RelRc`] object
/// is created, both its value as well as its parents cannot be changed. Children
/// can however always be added (and removed when falling out of scope).
#[derive(Debug)]
pub struct RelRc<N, E>(Rc<InnerData<N, E>>);

impl<N, E> Clone for RelRc<N, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N, E> RelRc<N, E> {
    /// Create a new [`RelRc<N, E>`] with no parents.
    pub fn new(value: N) -> Self {
        Self(Rc::new(InnerData::new(value)))
    }

    /// Create a new [`RelRc<N, E>`] with the given list of parent objects.
    ///
    /// The parents must be given by an object [`RelRc<N, E>`] and its
    /// corresponding edge value. The order of the parents is guaranteed to
    /// never change.
    pub fn with_parents(value: N, parents: impl IntoIterator<Item = (RelRc<N, E>, E)>) -> Self {
        let node = RelRc(Rc::new_cyclic(|weak_node| {
            let weak_node: RelWeak<N, E> = weak_node.clone().into();
            let incoming = parents
                .into_iter()
                .map(|(parent, edge_value)| {
                    InnerEdgeData::new(edge_value, parent, weak_node.clone())
                })
                .collect();
            InnerData::with_incoming(value, incoming)
        }));
        register_outgoing_edges(&node.incoming);
        node
    }

    /// Get a raw pointer to the underlying data.
    ///
    /// This is a low-level function that returns a raw pointer to the
    /// underlying data. The pointer is valid as long as at least one reference
    /// to the data exists.
    pub fn as_ptr(this: &Self) -> *const InnerData<N, E> {
        Rc::as_ptr(&this.0)
    }

    /// Unwrap the pointer, returning the value if `self` was the only owner.
    ///
    /// Returns an Err with `self` if there is more than one owner.
    pub fn try_unwrap(this: Self) -> Result<N, Self> {
        match Rc::try_unwrap(this.0) {
            Ok(data) => Ok(data.value),
            Err(data) => Err(RelRc(data)),
        }
    }

    /// Check if two pointers point to the same underlying data by comparing their
    /// raw pointers.
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(&this.0, &other.0)
    }
}

/// A weak reference to a [`RelRc`] object.
///
/// Upgrades to [`RelRc`] if the reference is valid.
#[derive(Debug, From)]
pub(crate) struct RelWeak<N, E>(Weak<InnerData<N, E>>);

impl<N, E> Clone for RelWeak<N, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N, E> RelWeak<N, E> {
    /// Upgrades to a [`Node`] if the reference is still valid.
    pub fn upgrade(&self) -> Option<RelRc<N, E>> {
        self.0.upgrade().map(RelRc)
    }
}

/// Data within a [`RelRc`] object.
///
/// Keeps track of its incident edges. Sole owner of the incoming edges, i.e. the
/// edges will exist if and only if the node exists. References to outgoing edges
/// are weak references, thus they may get deleted if all downstream nodes have
/// been deleted.
#[derive(Debug)]
pub struct InnerData<N, E> {
    /// The value of the node.
    value: N,
    /// The incoming edges to the object.
    ///
    /// The ordering and position of the incoming edges is immutable.
    incoming: Vec<InnerEdgeData<N, E>>,
    /// The outgoing edges from the object (weak references).
    ///
    /// The order and position of the outgoing edges may change at any time, as
    /// the edges may get deleted.
    outgoing: RefCell<Vec<WeakEdge<N, E>>>,
}

impl<N, E> Deref for RelRc<N, E> {
    type Target = InnerData<N, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N: Default, E> Default for InnerData<N, E> {
    fn default() -> Self {
        Self {
            value: Default::default(),
            incoming: Vec::new(),
            outgoing: RefCell::new(Vec::new()),
        }
    }
}

impl<N: Default, E> Default for RelRc<N, E> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<N, E> InnerData<N, E> {
    pub(crate) fn new(value: N) -> Self {
        Self {
            value,
            incoming: Vec::new(),
            outgoing: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn with_incoming(value: N, incoming: Vec<InnerEdgeData<N, E>>) -> Self {
        Self {
            value,
            incoming,
            outgoing: RefCell::new(Vec::new()),
        }
    }

    /// The i-th incoming edge to the node.
    pub fn incoming(&self, index: usize) -> Option<&InnerEdgeData<N, E>> {
        self.incoming.get(index)
    }

    /// The i-th parent of the object.
    pub fn parent(&self, index: usize) -> Option<&RelRc<N, E>> {
        self.incoming.get(index).map(|e| e.source())
    }

    /// The value of the object, also obtainable with [`Deref`].
    pub fn value(&self) -> &N {
        &self.value
    }

    /// All incoming edges as a slice.
    pub fn all_incoming(&self) -> &[InnerEdgeData<N, E>] {
        &self.incoming
    }

    /// Iterate over all outgoing edges.
    ///
    /// The edges are weakly referenced, so they may get deleted if all downstream
    /// nodes have been deleted.
    ///
    /// This upgrades all outgoing edges, removes references to edges that have
    /// been deleted, and returns the remaining edges in a new vector. This is
    /// not done lazily to limit mutable borrow of the outgoing RefCell.
    pub fn all_outgoing(&self) -> Vec<Edge<N, E>> {
        let mut edges = Vec::new();
        self.outgoing.borrow_mut().retain(|e| {
            if let Some(edge) = e.upgrade() {
                edges.push(edge);
                true
            } else {
                false
            }
        });
        edges
    }

    /// Iterate over all children of the object.
    ///
    /// The children are the objects that have an incoming edge from the object.
    pub fn all_children(&self) -> impl Iterator<Item = RelRc<N, E>> {
        self.all_outgoing().into_iter().map(|e| e.into_target())
    }

    /// The number of outgoing edges.
    pub fn n_outgoing(&self) -> usize {
        self.all_outgoing().len()
    }
}

fn register_outgoing_edges<N, E>(incoming: &[InnerEdgeData<N, E>]) {
    for (i, edge) in incoming.iter().enumerate() {
        edge.source().outgoing.borrow_mut().push(edge.downgrade(i));
    }
}
