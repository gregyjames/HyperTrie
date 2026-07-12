## 2026-05-10 - [Bloom Filter Hashing Optimization]
**Learning:** Re-hashing the base hash with a full hasher for every iteration in a Bloom Filter is significantly slower than using enhanced double hashing (hash_i = h1 + i * h2).
**Action:** Use double hashing to derive subsequent hashes in Bloom filters instead of expensive re-hashing.

## 2026-07-05 - [Case-Insensitive Trie Hashing and Stack Buffers]
**Learning:** Normalizing strings on the heap during hot paths (like Trie traversal or Bloom Filter checks) adds significant allocation overhead. Using a stack-allocated buffer (e.g., 64 bytes) combined with a precomputed `CHAR_TO_BIT` lookup table eliminates these allocations and branches. Additionally, ensuring consistency between the Trie normalization and Bloom Filter hashing is critical to avoid "false negatives" where a word is in the Trie but the Bloom Filter says it's not due to case mismatch.
**Action:** Use stack-allocated buffers and lookup tables for character normalization. Always normalize bytes before passing them to the Bloom Filter in case-insensitive Tries.

## 2026-07-12 - [Arena-Backed Slow Path Normalization with Stumpalo]
**Learning:** General-purpose heap allocators (even highly-optimized ones like `mimalloc`) introduce overhead and fragmentation when handling frequent dynamic string/slice normalization for long strings (exceeding stack threshold). Incorporating `stumpalo` as a local stable-compatible bump allocator and holding a `Mutex<stumpalo::Arena>` per Trie instance allows us to completely recycle temporary memory allocations for long strings. By utilizing `.get_mut()` for `insert` (bypassing lock overhead when having exclusive ownership) and scoped `.lock()` for `contains`, we eliminate general-purpose heap allocations on the slow path, maintaining elite performance under all input distributions.
**Action:** Incorporate `stumpalo` to optimize temporary heap allocations, fallback to stable-compatible APIs on older rustc versions, and use deferred stable-lifetime bindings (`Option::insert`) to extend the lifetime of locked arena slices securely.
