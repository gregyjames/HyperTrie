## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-10-04 - [Direct Word Bitmask and Direct Bit Index Normalization]
**Learning:** External `bit_vec` abstractions add unnecessary function call overhead and bounds checks compared to a simple `Vec<u64>` bitmask array. Furthermore, storing raw 0..25 character bit indices directly in normalized string buffers eliminates redundant ASCII offset arithmetic (`+ b'a'` and `- b'a'`) in hot lookup loops.
**Action:** Use direct `Vec<u64>` for Bloom Filter bit arrays and store normalized character indices directly in hot traversal paths.
