# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased] - tbd

### Added

- Replaced `RelRcGraph` with a new `HistoryGraph`
- Generics `N` and `E` no longer require `Hash`. Instead implement `EquivalenceResolver` for your types to enable graph merging.
- `Detached` was removed. Use `RelRc::to_serialized` and `RelRc::from_serialized` to convert `RelRc` into a serializable format.
- New `RelRcGraph::from_sinks_while` function for more general graph construction.
- `RelRcGraph` no longer requires sources. Any induced subgraph of `RelRc` objects is now supported.
- `RelRc::all_children` and `RelRc::all_parents` now return `ExactSizeIterator`s.
- `RelRcGraph::map` to create a new graph with mapped node and edge weights.
- `RelRcGraph::outgoing_edges` to get all outgoing edge IDs from a node.
- `RelRc::detach` and `RelRc::attach` to attach and detach a `RelRc` object from its parents. This allows to "move" objects by detaching them from one graph and attaching them to another. In the future, will be used for serialisation.
- Every `RelRc` object now has a unique `RelRcHash` hash ID, retrievable via `RelRc::hash_id`. Similar to git's commit hashes.

### Changed

- `GraphView` is renamed to `RelRcGraph`.
- `RelRcGraph::merge` now takes a callback called on every node that is merged.
- Using instance methods instead of static functions for RelRc.
- `RelRc` requires `Hash` to construct, as hash values are computed and memoised at construction time.

### Removed

- `RelRcGraph::sources` was removed. If required traverse all nodes and filter for `n_incoming == 0`.

## [0.2.2] - 2024-08-08

### Added

- Implement `Default` for `GraphView`
- Implement `Serialize` and `Deserialize` for `GraphView`.

### Changed

- `GraphView::lowest_common_ancestors` takes a slice of `Borrow<Self>` as argument.

## [0.2.1] - 2024-08-05

### Fixed

- Restored `all_indices` util function for `petgraph` feature.

## [0.2.0] - 2024-08-05

### Added

- `GraphView::from_sources` to create a `GraphView` for all descendants.
- `GraphView::merge` to merge two `GraphView`s.
- `GraphView::lowest_common_ancestors` to find the lowest common ancestors of `GraphView`s.
- `GraphView` serialization (activate `serde` feature).

### Changed

- `AncestorGraph` is now called `GraphView`
- Use "sources" and "sinks" instead of "initials" and "terminals" in `GraphView`

## [0.1.1] - 2024-07-31

### Added

- Add `AncestorGraph` to walk the RelRc objects as graph.
- Add petgraph feature to traverse `AncestorGraph`s.

## [0.1.0] - 2024-07-31

### Added

- Initial release of the relrc crate
- RelRc: an Rc that can link to parent RelRc objects

### Changed

### Deprecated

### Removed

### Fixed

### Security
