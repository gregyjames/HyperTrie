## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-08-30 - [Direct Bit Indices and Additive Hash Derivation]
**Learning:** Storing `bit_idx` directly (0..25) in normalized buffers avoids redundant `+ b'a'` and `- b'a'` byte offset math during trie lookup/insertion hot paths. Additionally, replacing multiplication-based hash derivation `(i as u64).wrapping_mul(h2)` with iterative addition (`final_hash += h2`) in Bloom Filter loops reduces instruction latency on modern CPUs.
**Action:** Store direct 0..25 bit indices in normalized trie buffers and use additive updates for double hashing loops.
