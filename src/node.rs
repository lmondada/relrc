//! Reference-counted pointers.

use std::cell::Ref;
use std::collections::VecDeque;
use std::iter;
use std::{
    cell::RefCell,
    ops::Deref,
    rc::{Rc, Weak},
};

use derive_more::From;
use derive_where::derive_where;
use rustc_hash::FxHashSet;

use crate::Registry;
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
/// is created, both its value as well as its parents cannot be changed.
/// Children can however always be added (and removed when falling out of
/// scope).
///
/// ## Unique IDs
///
/// Every [`RelRc`] object is assigned a unique hash-based identifier. For this
/// reason, object creation operations will require N and E generics to be
/// hashable.
#[derive(Debug)]
#[derive_where(Clone)]
pub struct RelRc<N, E>(Rc<InnerData<N, E>>);

impl<N, E> From<Rc<InnerData<N, E>>> for RelRc<N, E> {
    fn from(inner: Rc<InnerData<N, E>>) -> Self {
        Self(inner)
    }
}

impl<N, E> RelRc<N, E> {
    /// Create a new [`RelRc<N, E>`] with no parents.
    pub fn new(value: N) -> Self {
        let inner = Rc::new(InnerData::new(value));
        inner.into()
    }

    /// Create a new [`RelRc<N, E>`] with the given list of parent objects.
    ///
    /// The parents must be given by an object [`RelRc<N, E>`] and its
    /// corresponding edge value. The order of the parents is guaranteed to
    /// never change.
    pub fn with_parents(value: N, parents: impl IntoIterator<Item = (RelRc<N, E>, E)>) -> Self {
        let inner = Rc::new_cyclic(|weak_node| {
            let weak_node: RelWeak<N, E> = weak_node.clone().into();
            let incoming = parents
                .into_iter()
                .map(|(parent, edge_value)| {
                    InnerEdgeData::new(edge_value, parent, weak_node.clone())
                })
                .collect();
            InnerData::with_incoming(value, incoming)
        });
        let node = Self::from(inner);
        register_outgoing_edges(&node.incoming);
        node
    }
}

impl<N, E> RelRc<N, E> {
    /// Get a raw pointer to the underlying data.
    ///
    /// This is a low-level function that returns a raw pointer to the
    /// underlying data. The pointer is valid as long as at least one reference
    /// to the data exists.
    pub fn as_ptr(&self) -> *const InnerData<N, E> {
        Rc::as_ptr(&self.0)
    }

    /// Check if two pointers point to the same underlying data by comparing
    /// their raw pointers.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Downgrade the node to a weak reference.
    pub fn downgrade(&self) -> RelWeak<N, E> {
        RelWeak(Rc::downgrade(&self.0))
    }

    /// Register this node in the given registry and return its ID.
    ///
    /// A node can only be registered in one registry at a time. If the node is
    /// already registered in a different registry, this will return `None`.
    ///
    /// The node ID in the registry will be freed when the last reference to
    /// `self` is dropped.
    #[must_use]
    pub fn try_register_in(
        &self,
        registry: &Rc<RefCell<Registry<N, E>>>,
    ) -> Option<crate::registry::NodeId> {
        if !self.try_set_register(registry) {
            return None;
        }
        Some(registry.borrow_mut().add_node(self))
    }

    /// Get the registry that this node is registered in, if there is one.
    pub fn registry(&self) -> Option<Rc<RefCell<Registry<N, E>>>> {
        self.0
            .registry
            .borrow()
            .as_ref()
            .and_then(|weak| weak.upgrade())
    }

    /// Set the registry that tracks this node.
    ///
    /// A node can only be registered in one registry at a time. If the node is
    /// already registered in a different registry, this will return `None`.
    ///
    /// The node ID in the registry will be freed when the last reference to
    /// `self` is dropped.
    #[must_use]
    fn try_set_register(&self, registry: &Rc<RefCell<Registry<N, E>>>) -> bool {
        let weak = Rc::downgrade(registry);
        if let Some(existing) = self.0.registry.borrow().as_ref() {
            return existing.ptr_eq(&weak);
        }
        self.0.registry.borrow_mut().replace(weak);
        true
    }

    /// Iterate over all ancestors of the object, including self.
    pub fn all_ancestors(&self) -> impl Iterator<Item = &RelRc<N, E>> + '_ {
        let mut seen = FxHashSet::default();
        let mut stack = VecDeque::from([self]);

        iter::from_fn(move || {
            let node = stack.pop_front()?;
            if !seen.insert(node.as_ptr()) {
                return None;
            }
            stack.extend(node.all_parents());
            Some(node)
        })
    }
}

impl<N, E> Drop for RelRc<N, E> {
    fn drop(&mut self) {
        if Rc::strong_count(&self.0) > 1 {
            return;
        }
        let mut register = self.0.registry.borrow_mut();
        let Some(weak) = register.take() else {
            return;
        };
        let Some(registry) = weak.upgrade() else {
            return;
        };
        let Some(id) = registry.borrow().get_id(self) else {
            return;
        };
        registry.borrow_mut().remove(id);
    }
}

/// A weak reference to a [`RelRc`] object.
///
/// Upgrades to [`RelRc`] if the reference is valid.
#[derive(Debug, From)]
#[derive_where(Clone)]
pub struct RelWeak<N, E>(Weak<InnerData<N, E>>);

impl<N, E> RelWeak<N, E> {
    /// Upgrades to a [`Node`] if the reference is still valid.
    pub fn upgrade(&self) -> Option<RelRc<N, E>> {
        self.0.upgrade().map(RelRc::from)
    }
}

impl<N, E> RelWeak<N, E> {
    /// Check if two weak references point to the same underlying data
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }

    /// Get a raw pointer to the underlying data.
    pub fn as_ptr(&self) -> *const InnerData<N, E> {
        Weak::as_ptr(&self.0)
    }
}

/// A weak reference to a [`Registry`] object.
pub type WeakRegistry<N, E> = Weak<RefCell<Registry<N, E>>>;

/// Data within a [`RelRc`] object.
///
/// Keeps track of its incident edges. Sole owner of the incoming edges, i.e.
/// the edges will exist if and only if the node exists. References to outgoing
/// edges are weak references, thus they may get deleted if all downstream nodes
/// have been deleted.
#[derive(Debug, Clone)]
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
    /// The registry that tracks this node, if there is one.
    registry: RefCell<Option<WeakRegistry<N, E>>>,
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
            registry: RefCell::new(None),
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
            registry: RefCell::new(None),
        }
    }

    pub(crate) fn with_incoming(value: N, incoming: Vec<InnerEdgeData<N, E>>) -> Self {
        Self {
            value,
            incoming,
            outgoing: RefCell::new(Vec::new()),
            registry: RefCell::new(None),
        }
    }

    /// The i-th incoming edge to the node.
    pub fn incoming(&self, index: usize) -> Option<&InnerEdgeData<N, E>> {
        self.incoming.get(index)
    }

    /// The i-th incoming edge to the node as a weak reference.
    pub fn incoming_weak(&self, index: usize) -> Option<WeakEdge<N, E>> {
        self.incoming
            .get(index)
            .map(|e| WeakEdge::new(index, e.target.clone()))
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

    /// All incoming edges as weak references.
    pub fn all_incoming_weak(&self) -> impl ExactSizeIterator<Item = WeakEdge<N, E>> + '_ {
        self.all_incoming()
            .iter()
            .enumerate()
            .map(|(i, e)| WeakEdge::new(i, e.target.clone()))
    }

    /// All outgoing edges as weak references.
    ///
    /// This makes a borrow to the underlying `RefCell`, meaning that no
    /// new RelRc objects can be created for as long as the Ref is in scope.
    ///
    /// Don't expose publicly to avoid this issue.
    pub(crate) fn all_outgoing_weak_ref(&self) -> Ref<'_, [WeakEdge<N, E>]> {
        Ref::map(self.outgoing.borrow(), |edges| edges.as_slice())
    }

    /// All outgoing edges as weak references.
    pub fn all_outgoing_weak(&self) -> Vec<WeakEdge<N, E>> {
        self.all_outgoing_weak_ref().to_vec()
    }

    /// Iterate over all parents of the object.
    pub fn all_parents(&self) -> impl ExactSizeIterator<Item = &RelRc<N, E>> {
        self.all_incoming().iter().map(|e| e.source())
    }

    /// The number of incoming edges.
    pub fn n_incoming(&self) -> usize {
        self.incoming.len()
    }
}

impl<N, E> InnerData<N, E> {
    /// Iterate over all outgoing edges.
    ///
    /// The edges are weakly referenced, so they may get deleted if all
    /// downstream nodes have been deleted.
    ///
    /// This upgrades all outgoing edges, removes references to edges that have
    /// been deleted, and returns the remaining edges in a new vector. This is
    /// not done lazily to limit mutable borrow of the outgoing RefCell.
    pub fn all_outgoing(&self) -> Vec<Edge<N, E>> {
        let mut edges = Vec::with_capacity(self.outgoing.borrow().len());
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
    pub fn all_children(&self) -> impl ExactSizeIterator<Item = RelRc<N, E>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_dead_references() {
        let registry = Rc::new(RefCell::new(Registry::<&str, ()>::new()));

        let id = {
            let node = RelRc::new("test");
            let id = node.try_register_in(&registry).unwrap();

            assert_eq!(registry.borrow().len(), 1);
            id
        }; // node is dropped here

        let registry = Rc::try_unwrap(registry).unwrap().into_inner();

        // Should return None and clean up the entry
        assert!(registry.get(id).is_none());
        assert!(!registry.contains_id(id));
        assert_eq!(registry.len(), 0);
    }
}
