#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod ancestor_graph;
pub mod edge;
pub mod node;
#[cfg(feature = "petgraph")]
pub mod petgraph;

pub use ancestor_graph::{AncestorGraph, EdgeId, NodeId};
pub use edge::Edge;
pub use node::RelRc;

// Weak references are not exported publicly.
pub(crate) use edge::WeakEdge;
pub(crate) use node::RelWeak;
