# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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