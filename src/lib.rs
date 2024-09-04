#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod edge;
pub mod graph_view;
pub mod node;
#[cfg(feature = "petgraph")]
pub mod petgraph;

pub use edge::Edge;
pub use graph_view::{EdgeId, RelRcGraph, NodeId};
pub use node::RelRc;

// Weak references are not exported publicly.
pub(crate) use edge::WeakEdge;
pub(crate) use node::RelWeak;
