## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-10-11 - [Direct Bitset Array and Compact Node Encoding]
**Learning:** Replacing third-party bit vector abstractions (`bit-vec`) with a direct `Vec<u64>` bitset eliminates bounds checks and method wrapper overhead in Bloom Filter hot loops. In addition, packing node boolean state (`end_of_word`) into unused bitwise flags (bit 31 of child mask) reduces node struct memory footprint and enables branchless traversal using `trailing_zeros()`.
**Action:** Prefer primitive word-aligned arrays (`u64`) and bit-flag encodings for compact data structures in Rust hot paths.
