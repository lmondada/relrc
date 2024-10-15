#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

pub mod detached;
pub mod edge;
pub mod graph_view;
pub mod hash_id;
pub mod node;
#[cfg(feature = "petgraph")]
pub mod petgraph;

pub use edge::Edge;
pub use graph_view::{EdgeId, NodeId, RelRcGraph};
pub use node::RelRc;

// Weak references are not exported publicly.
pub(crate) use edge::WeakEdge;
pub(crate) use node::RelWeak;
