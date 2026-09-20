## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-09-20 - [Direct Bit-Index Normalization and Compact Trie Node Bitmasks]
**Learning:** Storing direct 0..25 bit indices in character normalization buffers instead of ASCII characters (`b'a' + bit_idx`) eliminates runtime addition and subtraction math during Trie traversal loops. In addition, packing boolean state (such as `end_of_word`) into unused bits of existing `u32` bitmasks reduces node memory footprint (from 112 bytes to 108 bytes), improving CPU cache density for large prefix trees.
**Action:** Store raw normalized indices directly in normalization buffers and leverage unused bitmask bits for boolean flags in node structs.
