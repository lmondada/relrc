//! A map function to change the node types of a [`RelRcGraph`].

use std::collections::BTreeMap;
use std::hash::Hash;

use petgraph::{
    algo::toposort,
    visit::{EdgeRef, IntoEdgesDirected},
    Direction,
};

use crate::RelRc;

use super::RelRcGraph;

impl<N: Hash, E: Hash> RelRcGraph<N, E> {
    /// Apply a map function to the nodes of the graph.
    ///
    /// Note the map function cannot take the node weights by value, since it
    /// cannot be guaranteed that the graph is the sole owner of the nodes.
    pub fn map<M: Hash, F: Hash>(
        &self,
        map_node: impl Fn(&N) -> M,
        map_edge: impl Fn(&E) -> F,
    ) -> RelRcGraph<M, F> {
        let mut rc_map: BTreeMap<_, RelRc<M, F>> = BTreeMap::new();

        for node_id in toposort(&self, None).unwrap() {
            let value = self.get_node(node_id).value();
            let new_value = map_node(value);
            let parents = self
                .edges_directed(node_id, Direction::Incoming)
                .map(|e| (rc_map[&e.source()].clone(), map_edge(e.weight())));
            let new_node = RelRc::with_parents(new_value, parents);
            rc_map.insert(node_id, new_node);
        }

        RelRcGraph::from_sinks(
            self.sinks()
                .iter()
                .map(|s| rc_map[&s.into()].clone())
                .collect(),
        )
    }
}
