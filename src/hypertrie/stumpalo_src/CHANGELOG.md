# Changelog

## [Unreleased]

### Added
- `alloc_layout` method.

## [0.2.1]

### Fixed
- Don't call `drop` on values produced by `alloc_sized_slice_with`.

## [0.2.0] - 2026-06-04

### Added
- `Arena` now implements `Send`, enabling cross-thread transfer.

## Pre-0.2.0

Initial public release.

### Added
- Fast bump allocation — the hot path is as few as six instructions
  with one conditional branch.
- Chunk reuse via `.clear()`, which resets the arena without freeing
  underlying memory so chunks are recycled on subsequent allocations.
- Scoped stack support via `with_scope()`, creating a temporary
  sub-arena whose allocations live only for the duration of a closure.
- String and slice allocation helpers.
- Configurable default chunk size.
- Incremental chunk size growth.
- Benchmark suite using, comparing against `bumpalo` and `blink-alloc`.
- `no_std`-compatibile.


[Unreleased]: https://codeberg.org/414owen/stumpalo/compare/v0.2.0...HEAD
[0.2.0]: https://codeberg.org/414owen/stumpalo/releases/tag/v0.2.0
