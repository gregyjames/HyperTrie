use bit_vec::BitVec;
use gxhash::GxHasher;
use std::hash::Hasher;

const SEED: i64 = 1846279233212321312;

pub struct BloomFilter {
    bit_array: BitVec,
    size: usize,
    num_hashes: usize,
}

impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        BloomFilter {
            bit_array: BitVec::from_elem(size, false),
            size,
            num_hashes,
        }
    }

    pub fn insert_bytes(&mut self, item: &[u8]) {
        let base_hash = self.get_base_hash(item);
        for i in 0..self.num_hashes {
            let final_hash = self.derive_hash(base_hash, i);
            let index = final_hash & (self.size - 1);
            self.bit_array.set(index, true);
        }
    }

    pub fn contains_bytes(&self, item: &[u8]) -> bool {
        let base_hash = self.get_base_hash(item);
        for i in 0..self.num_hashes {
            let final_hash = self.derive_hash(base_hash, i);
            let index = final_hash & (self.size - 1);

            if !self.bit_array.get(index).unwrap_or(false) {
                return false;
            }
        }
        true // Maybe in the set (false positives possible)
    }

    #[inline(always)]
    fn get_base_hash(&self, item: &[u8]) -> u64 {
        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(item);
        hasher.finish()
    }

    /// Derive subsequent hashes from the base hash + index
    /// This is significantly faster than hashing the string again
    #[inline(always)]
    fn derive_hash(&self, base_hash: u64, index: usize) -> usize {
        // Enhanced Double Hashing: hash_i = hash1 + i * hash2
        // We use the base_hash as hash1, and a secondary hash of base_hash as hash2
        let hash2 = base_hash.wrapping_mul(0x9e3779b97f4a7c15);
        base_hash.wrapping_add((index as u64).wrapping_mul(hash2)) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_filter(size: usize, num_hashes: usize) -> BloomFilter {
        BloomFilter::new(size, num_hashes)
    }

    // --- construction ---

    #[test]
    fn test_new_contains_nothing() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains_bytes(b"hello"));
        assert!(!bf.contains_bytes(b"world"));
    }

    #[test]
    fn test_new_size_one() {
        // degenerate but shouldn't panic
        let mut bf = make_filter(1, 1);
        bf.insert_bytes(b"x");
        assert!(bf.contains_bytes(b"x"));
    }

    // --- insert / contains ---

    #[test]
    fn test_inserted_item_is_found() {
        let mut bf = make_filter(1024, 3);
        bf.insert_bytes(b"hello");
        assert!(bf.contains_bytes(b"hello"));
    }

    #[test]
    fn test_multiple_insertions() {
        let mut bf = make_filter(1024, 3);
        let words = ["apple", "banana", "cherry", "date", "elderberry"];
        for w in &words {
            bf.insert_bytes(w.as_bytes());
        }
        for w in &words {
            assert!(
                bf.contains_bytes(w.as_bytes()),
                "expected '{w}' to be found"
            );
        }
    }

    #[test]
    fn test_non_inserted_item_likely_absent() {
        // With a large filter and few items, false positives should not occur
        // for these specific values — if this ever flakes, increase size.
        let mut bf = make_filter(8192, 4);
        bf.insert_bytes(b"present");
        assert!(!bf.contains_bytes(b"absent"));
        assert!(!bf.contains_bytes(b"also_absent"));
    }

    #[test]
    fn test_insert_empty_string() {
        let mut bf = make_filter(1024, 3);
        bf.insert_bytes(b"");
        assert!(bf.contains_bytes(b""));
    }

    #[test]
    fn test_empty_string_not_present_by_default() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains_bytes(b""));
    }

    #[test]
    fn test_insert_is_idempotent() {
        let mut bf = make_filter(1024, 3);
        bf.insert_bytes(b"repeat");
        bf.insert_bytes(b"repeat");
        assert!(bf.contains_bytes(b"repeat"));
    }

    #[test]
    fn test_unicode_item() {
        let mut bf = make_filter(1024, 3);
        bf.insert_bytes("héllo".as_bytes());
        bf.insert_bytes("日本語".as_bytes());
        assert!(bf.contains_bytes("héllo".as_bytes()));
        assert!(bf.contains_bytes("日本語".as_bytes()));
        assert!(!bf.contains_bytes(b"hello")); // ASCII variant is distinct
    }

    #[test]
    fn test_case_sensitive() {
        let mut bf = make_filter(1024, 3);
        bf.insert_bytes(b"Hello");
        assert!(bf.contains_bytes(b"Hello"));
        assert!(!bf.contains_bytes(b"hello"));
        assert!(!bf.contains_bytes(b"HELLO"));
    }

    #[test]
    fn test_similar_strings_are_distinct() {
        let mut bf = make_filter(8192, 4);
        bf.insert_bytes(b"abc");
        assert!(!bf.contains_bytes(b"ab"));
        assert!(!bf.contains_bytes(b"abcd"));
        assert!(!bf.contains_bytes(b"ABC"));
    }

    // --- num_hashes boundary ---

    #[test]
    fn test_single_hash() {
        let mut bf = make_filter(1024, 1);
        bf.insert_bytes(b"one_hash");
        assert!(bf.contains_bytes(b"one_hash"));
        assert!(!bf.contains_bytes(b"different"));
    }

    #[test]
    fn test_many_hashes() {
        let mut bf = make_filter(4096, 10);
        bf.insert_bytes(b"many_hashes");
        assert!(bf.contains_bytes(b"many_hashes"));
    }

    // --- hash_item determinism ---
    /// Helper to collect hashes for testing since we removed hash_item from the API
    fn get_hashes(bf: &BloomFilter, item: &str) -> Vec<usize> {
        let mut hashes = Vec::new();
        let base_hash = bf.get_base_hash(item.as_bytes());
        for i in 0..bf.num_hashes {
            let final_hash = bf.derive_hash(base_hash, i);
            hashes.push(final_hash % bf.size);
        }
        hashes
    }

    #[test]
    fn test_hashing_is_deterministic() {
        let bf = make_filter(1024, 3);
        let h1 = get_hashes(&bf, "stable");
        let h2 = get_hashes(&bf, "stable");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_salts_produce_different_values() {
        // Each of the num_hashes slots should (almost certainly) differ for a
        // non-degenerate input, confirming the per-index salt is applied.
        let bf = make_filter(1024, 4);
        let hashes = get_hashes(&bf, "salt_test");
        // Not all hashes should be equal — if they were, the filter would
        // only ever set/check one bit position regardless of num_hashes.
        let unique: std::collections::HashSet<usize> = hashes.iter().copied().collect();
        assert!(unique.len() > 1, "expected distinct hash values per slot");
    }

    #[test]
    fn test_hash_count_matches_num_hashes() {
        let bf = make_filter(1024, 5);
        let hashes = get_hashes(&bf, "count");
        assert_eq!(hashes.len(), 5, "Should generate exactly num_hashes values");
    }

    // --- false positive rate sanity check ---

    #[test]
    fn test_false_positive_rate_is_reasonable() {
        // Insert 100 items into a well-sized filter, then probe 1000 items
        // that were never inserted. FP rate should be well under 1%.
        let mut bf = make_filter(16384, 4);
        for i in 0..100u32 {
            bf.insert_bytes(format!("inserted_{i}").as_bytes());
        }
        let mut false_positives = 0;
        for i in 0..1000u32 {
            if bf.contains_bytes(format!("probe_{i}").as_bytes()) {
                false_positives += 1;
            }
        }
        assert!(
            false_positives < 10,
            "false positive rate too high: {false_positives}/1000"
        );
    }
}
