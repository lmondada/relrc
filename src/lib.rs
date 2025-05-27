#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

// pub mod detached;
pub mod edge;
pub mod history;
pub mod node;
#[cfg(feature = "petgraph")]
pub mod petgraph;
pub mod resolver;
pub mod serialization;

pub use edge::Edge;
pub use history::{EdgeId, HistoryGraph, NodeId};
pub use node::RelRc;

// #[cfg(feature = "mpi")]
// pub use detached::mpi;

pub use edge::WeakEdge;
pub use node::RelWeak;

pub use resolver::EquivalenceResolver;
