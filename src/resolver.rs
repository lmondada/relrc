//! Define equivalence between `RelRc` instances, useful for merging RelRc
//! graphs and detached instances.

use std::hash::Hash;

use derive_where::derive_where;
use thiserror::Error;

/// Error type for when two vertices are not equivalent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Error)]
#[error("Vertices are not equivalent")]
pub struct NotEquivalent;

/// Define application-specific logic for deduplicating and merging node values
/// based on semantic equivalence of their data and incoming edges.
pub trait EquivalenceResolver<N, E> {
    /// Represent the information needed to map node identifiers between
    /// equivalent values.
    ///
    /// Used to translate edge sources and possibly internal references within
    /// edge values when merging nodes.
    type MergeMapping;

    /// Represent a coarse key used for grouping potentially equivalent nodes.
    ///
    /// Keys must be deterministic and hashable. False positives are allowed but
    /// incur a performance hit; false negatives will reduce deduplication
    /// effectiveness.
    type DedupKey: Eq + Hash;

    /// A unique identifier for the resolver.
    fn id(&self) -> String;

    /// Compute a deduplication key from a node value and its incoming edges.
    ///
    /// Use this to group nodes for equivalence checks. Nodes with the same key
    /// are considered candidates for merging.
    fn dedup_key(&self, value: &N, incoming_edges: &[&E]) -> Self::DedupKey;

    /// Determine whether two [`crate::RelRc`] are equivalent and if so,
    /// construct its corresponding merge mapping.
    ///
    /// Equivalence must be determined based on the [`crate::RelRc`] node
    /// values and their immediate incoming edges.
    ///
    /// The merge mapping is used to modify the edge values when the source
    /// of an edge `a` is moved to the equivalent node `b`. See
    /// [`EquivalenceResolver::move_edge_source`].
    ///
    /// The edge sources of `a_incoming_edges` and `b_incoming_edges` are
    /// pairwise identical, i.e. if the i-th edge of `a` is `p -> a`, then
    /// the i-th edge of `b` is `p -> b`.
    fn try_merge_mapping(
        &self,
        a_value: &N,
        a_incoming_edges: &[&E],
        b_value: &N,
        b_incoming_edges: &[&E],
    ) -> Result<Self::MergeMapping, NotEquivalent>;

    /// Update an edge value after moving its source node.
    ///
    /// If `v` is the source node of `edge`, then `mapping` describes an
    /// equivalence v -> v'. The returned edge value is the value of the edge
    /// that is equivalent to `edge` and starts at `v'`.
    fn move_edge_source(&self, mapping: &Self::MergeMapping, edge: &E) -> E;
}

/// Store a unique identifier for the resolver.
#[derive_where(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(bound = ""))]
pub struct ResolverId<N, E, R> {
    id: String,
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: std::marker::PhantomData<(N, E, R)>,
}

// impl<N, E, R> ResolverId<N, E, R> {
//     /// Cast the resolver ID to a different type.
//     pub(crate) fn cast<N2, E2, R2>(self) -> ResolverId<N2, E2, R2> {
//         ResolverId {
//             id: self.id,
//             _marker: std::marker::PhantomData,
//         }
//     }
// }

impl<'r, N, E, R: EquivalenceResolver<N, E>> From<&'r R> for ResolverId<N, E, R> {
    fn from(value: &'r R) -> Self {
        Self {
            id: value.id(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<N, E, R> From<ResolverId<N, E, R>> for String {
    fn from(value: ResolverId<N, E, R>) -> Self {
        value.id
    }
}

/// Error type when resolver IDs mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Error)]
#[error("Invalid resolver \"{0}\", expected \"{1}\"")]
pub struct InvalidResolver(pub String, pub String);

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A test resolver for N = (usize, usize) and E = usize.
    /// Two nodes are equivalent if the first element of their tuple is
    /// identical. All outgoing edges must have weight equal to the second
    /// element of the node's tuple.
    #[derive(Debug, Copy, Clone, Default)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub(crate) struct TestResolver;

    impl EquivalenceResolver<(usize, usize), usize> for TestResolver {
        type DedupKey = usize;
        type MergeMapping = usize; // We only need to store the second element of the tuple

        fn id(&self) -> String {
            "test_resolver".to_string()
        }

        fn dedup_key(&self, value: &(usize, usize), _incoming_edges: &[&usize]) -> Self::DedupKey {
            // Two nodes are equivalent if their first element is identical
            value.0
        }

        fn try_merge_mapping(
            &self,
            a_value: &(usize, usize),
            _a_incoming_edges: &[&usize],
            b_value: &(usize, usize),
            _b_incoming_edges: &[&usize],
        ) -> Result<Self::MergeMapping, NotEquivalent> {
            // Check if the first elements are identical
            if a_value.0 == b_value.0 {
                // Return the second element as the merge mapping
                Ok(b_value.1)
            } else {
                Err(NotEquivalent)
            }
        }

        fn move_edge_source(&self, mapping: &Self::MergeMapping, _edge: &usize) -> usize {
            // Update the edge value to match the second element of the target node
            *mapping
        }
    }

    #[test]
    fn test_resolver_equivalence() {
        let resolver = TestResolver;

        // Equivalent nodes (same first element)
        let a_value = (1, 2);
        let b_value = (1, 3);

        let result = resolver.try_merge_mapping(&a_value, &[], &b_value, &[]);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 3);

        // Non-equivalent nodes (different first element)
        let c_value = (2, 2);
        let result = resolver.try_merge_mapping(&a_value, &[], &c_value, &[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolver_edge_update() {
        let resolver = TestResolver;

        // Create a merge mapping (second element of target node)
        let mapping = 5;

        // Original edge value doesn't matter, it will be replaced with the mapping
        let edge = 10;

        // Edge should be updated to the second element of the target node
        assert_eq!(resolver.move_edge_source(&mapping, &edge), 5);
    }
}
