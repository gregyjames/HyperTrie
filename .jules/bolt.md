## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-06-14 - [Trie and FFI Boundary Optimizations]
**Learning:** Branchless character mapping using a static lookup table is faster than runtime case conversion. Passing string lengths across the FFI boundary avoids redundant null-terminator scans in Rust. Pre-calculating secondary hashes in Bloom Filters reduces redundant arithmetic in hot loops.
**Action:** Always prefer precomputed tables for character mapping and pass lengths explicitly across FFI boundaries for performance-critical code.
