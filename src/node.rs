use std::{
    cell::RefCell,
    ops::Deref,
    rc::{Rc, Weak},
};

use derive_more::From;

use crate::{edge::EdgeData, Edge, WeakEdge};

/// A node in a directed acylic graph.
#[derive(Debug)]
pub struct Node<N, E>(Rc<NodeData<N, E>>);

impl<N, E> Clone for Node<N, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N, E> Node<N, E> {
    /// Create a new node with no parents.
    pub fn new(value: N) -> Self {
        Self(Rc::new(NodeData::new(value)))
    }

    /// Create a new node with a list of incoming edges.
    ///
    /// Incoming edges are given by a parent [`Node`] and its corresponding edge
    /// weight. The order of the incoming edges is preserved.
    pub fn with_incoming(value: N, incoming: impl IntoIterator<Item = (Node<N, E>, E)>) -> Self {
        let node = Node(Rc::new_cyclic(|weak_node| {
            let weak_node: WeakNode<N, E> = weak_node.clone().into();
            let incoming = incoming
                .into_iter()
                .map(|(parent, edge_value)| EdgeData::new(edge_value, parent, weak_node.clone()))
                .collect();
            NodeData::with_incoming(value, incoming)
        }));
        register_outgoing_edges(&node.incoming);
        node
    }
}

/// A weak reference to a node.
///
/// Upgrades to [`Node`] if the reference is valid.
#[derive(Debug, From)]
pub(crate) struct WeakNode<N, E>(Weak<NodeData<N, E>>);

impl<N, E> Clone for WeakNode<N, E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<N, E> WeakNode<N, E> {
    /// Upgrades to a [`Node`] if the reference is still valid.
    pub fn upgrade(&self) -> Option<Node<N, E>> {
        self.0.upgrade().map(Node)
    }
}

/// A node in a directed acyclic graph. Always wrapped in an Rc.
///
/// Keeps references to its incident edges. References to incoming edges are
/// strong references, i.e. the edges will exist as long as the node exists.
/// References to outgoing edges on the other hand are weak references, thus
/// they may get deleted if all downstream nodes have been deleted.
#[derive(Debug)]
pub struct NodeData<N, E> {
    /// The value of the node.
    value: N,
    /// The incoming edges to the node (strong references).
    ///
    /// The ordering and position of the incoming edges is immutable.
    incoming: Vec<EdgeData<N, E>>,
    /// The outgoing edges from the node (weak references).
    ///
    /// The order and position of the outgoing edges may change at any time, as
    /// the edges may get deleted.
    outgoing: RefCell<Vec<WeakEdge<N, E>>>,
}

impl<N, E> Deref for Node<N, E> {
    type Target = NodeData<N, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<N, E> NodeData<N, E> {
    pub(crate) fn new(value: N) -> Self {
        Self {
            value,
            incoming: Vec::new(),
            outgoing: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn with_incoming(value: N, incoming: Vec<EdgeData<N, E>>) -> Self {
        Self {
            value,
            incoming,
            outgoing: RefCell::new(Vec::new()),
        }
    }

    /// The i-th incoming edge to the node.
    pub fn incoming(&self, index: usize) -> Option<&EdgeData<N, E>> {
        self.incoming.get(index)
    }

    /// The node weight.
    pub fn value(&self) -> &N {
        &self.value
    }

    /// All incoming edges as a slice.
    pub fn all_incoming(&self) -> &[EdgeData<N, E>] {
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

    /// The number of outgoing edges.
    pub fn n_outgoing(&self) -> usize {
        self.all_outgoing().len()
    }
}

fn register_outgoing_edges<N, E>(incoming: &[EdgeData<N, E>]) {
    for (i, edge) in incoming.iter().enumerate() {
        edge.source().outgoing.borrow_mut().push(edge.downgrade(i));
    }
}
