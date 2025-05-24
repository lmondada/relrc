# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.4.2 - 2025-05-24

### Added

- Weaken `serde` bounds on `HistoryGraph`

## 0.4.2 - 2025-05-24

### Added

- Add `NodeId::from_str` and `Display` implementation
- Add deserialization of `HistoryGraph`

## 0.4.1 - 2025-04-17

### Added

- Add HistoryGraph::{contains, contains_id}
- Add HistoryGraph::children

### Changed

- Two values will always be considered equivalent if they point to the same data.
- Relaxed petgraph dependency

## 0.4.0 - 2025-04-12

### Added

- Serialisation. Use `RelRc::to_serialized` and `RelRc::from_serialized` to convert `RelRc` into a serializable format.
- There is a notion of semantic equivalence between nodes. Implement `EquivalenceResolver` for your types to enable deduplication of nodes in `HistoryGraph`.

### Changed

- Replaced `GraphView` with a new `HistoryGraph`
- Using instance methods instead of static functions for `RelRc`.

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
