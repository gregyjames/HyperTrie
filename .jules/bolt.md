## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-10-18 - [Direct Vec<u64> Bit Arrays and Offset-Free Normalization]
**Learning:** External bit vector abstractions (like `bit-vec`) introduce bounds checking and method dispatch overhead in hot loops. Using a raw `Vec<u64>` bitmask array with power-of-two size masking and direct `gxhash::gxhash64` calls eliminates these overheads. Furthermore, storing 0-indexed bit indices directly in normalized string buffers avoids repeated ASCII offset calculations (`+ b'a'` and `- b'a'`) during trie traversal.
**Action:** Prefer direct `Vec<u64>` bit manipulation with unchecked indexing for power-of-two sized bloom filters, and pass 0-indexed byte slices to avoid ASCII offset math in hot paths.
