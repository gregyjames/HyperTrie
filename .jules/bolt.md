## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.
