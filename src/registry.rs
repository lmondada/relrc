//! Node registry for assigning unique IDs to RelRc nodes.

use derive_where::derive_where;
use slotmap::{new_key_type, SlotMap};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::RelWeak;
use crate::{node::InnerData, RelRc};

new_key_type! {
    /// A unique identifier for a node in the registry.
    pub struct NodeId;
}

/// Shared registry for assigning unique IDs to RelRc nodes.
///
/// Multiple graphs can share the same registry to maintain consistent
/// node identification across graphs.
#[derive(Debug)]
#[derive_where(Clone, Default)]
pub struct Registry<N, E> {
    /// Map from NodeId to weak references to nodes
    nodes: SlotMap<NodeId, RelWeak<N, E>>,
    /// Inverse map from raw pointer to NodeId for fast lookups
    ptr_to_id: HashMap<*const InnerData<N, E>, NodeId>,
}

impl<N, E> Registry<N, E> {
    /// Create a new empty node registry.
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            ptr_to_id: HashMap::new(),
        }
    }

    /// Create a new registry from a slotmap of nodes.
    ///
    /// Only used internally, to recreate a [`Registry`] from its serialized
    /// format.
    pub(crate) fn from_slotmap(nodes: SlotMap<NodeId, RelWeak<N, E>>) -> Self {
        let ptr_to_id = nodes
            .iter()
            .map(|(id, weak_ref)| (weak_ref.as_ptr(), id))
            .collect();
        Self { nodes, ptr_to_id }
    }

    /// Add a RelRc node to the registry and return its unique ID.
    ///
    /// If the node is already in the registry, return the existing ID.
    ///
    /// Prefer registering the node using [RelRc::try_register_in] or
    /// [crate::HistoryGraph::insert_node], as these will also free the
    /// node ID when the node goes out of scope.
    pub fn add_node(&mut self, node: &RelRc<N, E>) -> NodeId {
        // Fast lookup using inverse map
        if let Some(existing_id) = self.get_id(node) {
            return existing_id; // node is already registered
        }

        // Register new node
        let weak_ref = node.downgrade();
        let id = self.nodes.insert(weak_ref);
        self.ptr_to_id.insert(node.as_ptr(), id);

        id
    }

    /// Get the NodeId for a RelRc node if it's registered.
    ///
    /// This operation is O(1) thanks to the inverse map.
    pub fn get_id(&self, node: &RelRc<N, E>) -> Option<NodeId> {
        self.ptr_to_id.get(&node.as_ptr()).copied()
    }

    /// Get the NodeId for a RelRc node if it's registered, otherwise add it
    /// to the registry.
    pub fn get_id_or_insert(&mut self, node: &RelRc<N, E>) -> NodeId {
        if let Some(id) = self.get_id(node) {
            id
        } else {
            self.add_node(node)
        }
    }

    /// Check if a [`RelRc`] is contained in the registry.
    pub fn contains(&self, node: &RelRc<N, E>) -> bool {
        self.get_id(node).is_some()
    }

    /// Get the [`RelRc`] node associated with the given ID.
    ///
    /// If the weak reference cannot be upgraded, none is returned.
    pub fn get(&self, id: NodeId) -> Option<RelRc<N, E>> {
        let weak_ref = self.nodes.get(id).cloned()?;

        weak_ref.upgrade()
    }

    /// Check if a node ID is registered.
    pub fn contains_id(&self, id: NodeId) -> bool {
        self.get(id).is_some()
    }

    /// Remove all dead references and return the number of active nodes.
    ///
    /// This is automatically done during `get()` calls, but can be called
    /// explicitly for bulk cleanup.
    pub fn free_node_ids(&mut self) -> usize {
        // Collect dead entries
        let mut dead_ids = Vec::new();
        let mut dead_ptrs = Vec::new();

        for (id, weak_ref) in self.nodes.iter() {
            if weak_ref.upgrade().is_none() {
                dead_ids.push(id);
                dead_ptrs.push(weak_ref.as_ptr());
            }
        }

        // Remove dead entries
        for id in dead_ids {
            self.nodes.remove(id);
        }
        for ptr in dead_ptrs {
            self.ptr_to_id.remove(&ptr);
        }

        self.nodes.len()
    }

    /// Get the number of registered nodes (including potentially dead ones).
    ///
    /// Call `live_count()` if you want only live nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get the number of live nodes without modifying the registry.
    ///
    /// This is less efficient than `cleanup()` as it doesn't remove dead
    /// entries.
    pub fn live_count(&mut self) -> usize {
        self.free_node_ids();
        self.len()
    }

    /// Remove a node from the registry.
    pub fn remove(&mut self, id: NodeId) {
        // Node was dropped, clean up both maps
        let weak_ref = self.nodes.remove(id);
        if let Some(weak_ref) = weak_ref {
            self.ptr_to_id.remove(&weak_ref.as_ptr());
        }
    }

    pub(crate) fn as_slotmap(&self) -> &SlotMap<NodeId, RelWeak<N, E>> {
        &self.nodes
    }
}

impl<'r, N, E> FromIterator<&'r RelRc<N, E>> for Registry<N, E> {
    fn from_iter<T: IntoIterator<Item = &'r RelRc<N, E>>>(iter: T) -> Self {
        let mut registry = Self::new();
        for node in iter {
            registry.add_node(node);
        }
        registry
    }
}

impl<N, E> From<Registry<N, E>> for Rc<RefCell<Registry<N, E>>> {
    fn from(registry: Registry<N, E>) -> Self {
        Rc::new(RefCell::new(registry))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RelRc;

    #[test]
    fn test_register_and_get() {
        let mut registry = Registry::<&str, ()>::new();
        let node = RelRc::new("test");

        let id = registry.add_node(&node);
        let retrieved = registry.get(id).unwrap();

        assert!(RelRc::ptr_eq(&node, &retrieved));
    }

    #[test]
    fn test_duplicate_registration() {
        let mut registry = Registry::<&str, ()>::new();
        let node = RelRc::new("test");

        let id1 = registry.add_node(&node);
        let id2 = registry.add_node(&node);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_get_id() {
        let mut registry = Registry::<&str, ()>::new();
        let node = RelRc::new("test");

        assert_eq!(registry.get_id(&node), None);

        let id = registry.add_node(&node);
        assert_eq!(registry.get_id(&node), Some(id));
    }
}
