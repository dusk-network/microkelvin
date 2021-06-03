# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed
Change the name of `BranchMutMappedMut` to `MappedBranchMut`

### Removed
Remove `path` from branch traversal, case now covered by `walk`

## [0.7.2] - 2021-06-01

### Added
Add `IntoIterator` to `MappedBranch` and `BranchMutMappedMut`
Add support for non-mutably dereferencing mutable mapped branches

## [0.7.1] - 2021-04-27

### Added

- Add public export of `walk::Walker` in lib.rs

## [0.7.0] - 2021-04-21

### Added

- Add `Walker` trait, to specify the way in which a `Branch` or `BranchMut` can be walked from a root node
- Add `Nth` trait, to construct a branch to the nth element of a `Compound` collection
- Add `MaxKey` and `Keyed` traits to keep track of the maximum keyed leaf in the collection
- Add the `levels` method to `Branch` to introspect the individual branch `Levels`
- Add the `path` constructor to `Branch` and `BranchMut` to traverse the collectiong along a specified path
- Added the `MappedBranch` to provide branches that only allow access to certain parts of its leaves
- Add implementation of `IntoIterator` for `Branch` and `BranchMut`
- Add `First` auto-trait to construct a `Branch` to the first element in a collection
- Add `LinkedList` implementation in tests

### Changed

- Change `canonical`/`canonical_derive` version from 0.5 to 0.6
- Refactor the `Annotation` trait into `Annotation` for the leaves, and `Combine` for the nodes
- Change the iterator on `Compound` to only iterate over populated subtrees or leaves

### Removed

- Remove the `Annotation` trait parameter on `Compound`, moving it to a generic on the type

## [0.6.0] - 2021-01-25

### Changed

- Change the library to use `alloc::vec` instead of `const-arrayvec`

### Removed

- Remove `const-arrayvec` as a dependency
- Remove `CanonArrayVec` type

## [0.5.8] - 2021-01-21

### Added

- Add `no_std` crate-level annotation

## [0.5.7] - 2021-01-21

### Added

- Add `Annotation` implementation for `()`

## [0.5.6] - 2021-01-19

### Removed

- Remove unused `no_std` and `feature(min_const_generics)`

## [0.5.5] - 2020-12-03
### Changed
- `Max<K>` should implement `PartialOrd<K>`

## [0.5.4] - 2020-12-03
### Changed
- Annotation impl of Max<K> should require `Borrow<Max<K>>`

## [0.5.3] - 2020-11-16
### Changed
- Use PartialOrd with K in Max<K>

## [0.5.2] - 2020-11-06
### Changed
- Canonical update to support hosted-only calls
- Unused associative feature removed

## [0.5.1] - 2020-10-30
### Added
- Cardinality reference implements Into<u64>

## [0.5.0] - 2020-10-28
### Changed
- Associative annotation as a feature

## [0.4.0] - 2020-10-26

### Added

- Add documentation for all public exports
- Add pub exports for various types

## [0.3.0] - 2020-10-26

### Added

- Add CI infrastructure
- Add branch introspection via `levels` method

### Changed

- Changed `Branch::len` to `Branch::depth`
- Changed the library to be no_std compatible

### Removed

- Remove `Associative` helper trait

## [0.2.0] - 2020-10-21

### Changed

- Change the `Annotation::op` method to take self by value

### Added
- Add LICENSE and copyright notices
- Add `Nth` trait for trees
- Add capacity to search through trees by walking
- Add `Branch` and `BranchMut`

## [0.1.0] - 2020-10-16

Initial

[Unreleased]: https://github.com/dusk-network/microkelvin/compare/v-0.7.2...HEAD
[0.7.2]: https://github.com/dusk-network/microkelvin/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/dusk-network/microkelvin/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/dusk-network/microkelvin/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/dusk-network/microkelvin/compare/v0.5.8...v0.6.0
[0.5.8]: https://github.com/dusk-network/microkelvin/compare/v0.5.7...v0.5.8
[0.5.7]: https://github.com/dusk-network/microkelvin/compare/v0.5.6...v0.5.7
[0.5.6]: https://github.com/dusk-network/microkelvin/compare/v0.5.5...v0.5.6
[0.5.5]: https://github.com/dusk-network/microkelvin/compare/v0.5.4...v0.5.5
[0.5.4]: https://github.com/dusk-network/microkelvin/compare/v0.5.3...v0.5.4
[0.5.3]: https://github.com/dusk-network/microkelvin/compare/v0.5.2...v0.5.3
[0.5.2]: https://github.com/dusk-network/microkelvin/compare/v0.5.1...v0.5.2
[0.5.1]: https://github.com/dusk-network/microkelvin/compare/v0.5.0...v0.5.1
[0.5.0]: https://github.com/dusk-network/microkelvin/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/dusk-network/microkelvin/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/dusk-network/microkelvin/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/dusk-network/microkelvin/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/dusk-network/microkelvin/releases/tag/v0.1.0
