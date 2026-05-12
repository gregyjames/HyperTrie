## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-05-12 - [FFI and Memory Layout Optimizations]
**Learning:**
1. **FFI Overhead:** Passing null-terminated strings via `CStr` in Rust involves an $O(N)$ scan. Passing explicit length via `*const u8` and `usize` is faster and allows using `std::str::from_utf8_unchecked` when safety is guaranteed by the caller.
2. **Cache Locality (SoA vs AoS):** A Structure of Arrays (SoA) layout for the Trie (separate vectors for masks and indices) significantly improves cache hits during traversal compared to an Array of Structures (AoS).
3. **Data Compaction:** Packing boolean flags (like `end_of_word`) into unused bits of existing fields (like `children_mask`) reduces the memory footprint per node.
4. **Branchless Mapping:** Using a precomputed lookup table for character-to-index mapping is faster than performing ASCII arithmetic and case conversion branches.
**Action:** Implemented SoA layout, explicit length FFI, and packed bitmasks for the Trie and Bloom Filter.
