## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-06-21 - [Branchless Character Normalization]
**Learning:** Character normalization (lowercase + filtering) in hot paths can be significantly optimized using a static lookup table (CHAR_TO_BIT) and stack-allocated buffers ([u8; 64]), avoiding both branch mispredictions and heap allocations.
**Action:** Implement branchless mapping and use fixed-size stack buffers for common string lengths to eliminate allocation overhead in Trie and Bloom Filter operations.
