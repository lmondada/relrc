#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod edge;
mod node;

pub use edge::Edge;
pub use node::Node;

// Weak references are not exported publicly.
pub(crate) use edge::WeakEdge;
pub(crate) use node::WeakNode;
