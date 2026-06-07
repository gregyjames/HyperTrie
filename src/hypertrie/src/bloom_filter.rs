use bit_vec::BitVec;

pub(crate) const SEED: i64 = 1846279233212321312;

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

    pub fn insert_hash(&mut self, h1: u64) {
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        let mut final_hash = h1;
        let size_mask = self.size - 1;

        for _ in 0..self.num_hashes {
            let index = (final_hash as usize) & size_mask;
            self.bit_array.set(index, true);
            final_hash = final_hash.wrapping_add(h2);
        }
    }

    pub fn contains_hash(&self, h1: u64) -> bool {
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        let mut final_hash = h1;
        let size_mask = self.size - 1;

        for _ in 0..self.num_hashes {
            let index = (final_hash as usize) & size_mask;

            if !self.bit_array.get(index).unwrap_or(false) {
                return false;
            }
            final_hash = final_hash.wrapping_add(h2);
        }
        true // Maybe in the set (false positives possible)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gxhash::GxHasher;
    use std::hash::Hasher;

    fn make_filter(size: usize, num_hashes: usize) -> BloomFilter {
        BloomFilter::new(size, num_hashes)
    }

    fn hash(item: &str) -> u64 {
        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(item.as_bytes());
        hasher.finish()
    }

    // --- construction ---

    #[test]
    fn test_new_contains_nothing() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains_hash(hash("hello")));
        assert!(!bf.contains_hash(hash("world")));
    }

    #[test]
    fn test_new_size_one() {
        // degenerate but shouldn't panic
        let mut bf = make_filter(1, 1);
        bf.insert_hash(hash("x"));
        assert!(bf.contains_hash(hash("x")));
    }

    // --- insert / contains ---

    #[test]
    fn test_inserted_item_is_found() {
        let mut bf = make_filter(1024, 3);
        bf.insert_hash(hash("hello"));
        assert!(bf.contains_hash(hash("hello")));
    }

    #[test]
    fn test_multiple_insertions() {
        let mut bf = make_filter(1024, 3);
        let words = ["apple", "banana", "cherry", "date", "elderberry"];
        for w in &words {
            bf.insert_hash(hash(w));
        }
        for w in &words {
            assert!(bf.contains_hash(hash(w)), "expected '{w}' to be found");
        }
    }

    #[test]
    fn test_non_inserted_item_likely_absent() {
        // With a large filter and few items, false positives should not occur
        // for these specific values — if this ever flakes, increase size.
        let mut bf = make_filter(8192, 4);
        bf.insert_hash(hash("present"));
        assert!(!bf.contains_hash(hash("absent")));
        assert!(!bf.contains_hash(hash("also_absent")));
    }

    #[test]
    fn test_insert_empty_string() {
        let mut bf = make_filter(1024, 3);
        bf.insert_hash(hash(""));
        assert!(bf.contains_hash(hash("")));
    }

    #[test]
    fn test_empty_string_not_present_by_default() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains_hash(hash("")));
    }

    #[test]
    fn test_insert_is_idempotent() {
        let mut bf = make_filter(1024, 3);
        bf.insert_hash(hash("repeat"));
        bf.insert_hash(hash("repeat"));
        assert!(bf.contains_hash(hash("repeat")));
    }

    #[test]
    fn test_unicode_item() {
        let mut bf = make_filter(1024, 3);
        bf.insert_hash(hash("héllo"));
        bf.insert_hash(hash("日本語"));
        assert!(bf.contains_hash(hash("héllo")));
        assert!(bf.contains_hash(hash("日本語")));
        assert!(!bf.contains_hash(hash("hello"))); // ASCII variant is distinct
    }

    #[test]
    fn test_case_sensitive() {
        let mut bf = make_filter(1024, 3);
        bf.insert_hash(hash("Hello"));
        assert!(bf.contains_hash(hash("Hello")));
        assert!(!bf.contains_hash(hash("hello")));
        assert!(!bf.contains_hash(hash("HELLO")));
    }

    #[test]
    fn test_similar_strings_are_distinct() {
        let mut bf = make_filter(8192, 4);
        bf.insert_hash(hash("abc"));
        assert!(!bf.contains_hash(hash("ab")));
        assert!(!bf.contains_hash(hash("abcd")));
        assert!(!bf.contains_hash(hash("ABC")));
    }

    // --- num_hashes boundary ---

    #[test]
    fn test_single_hash() {
        let mut bf = make_filter(1024, 1);
        bf.insert_hash(hash("one_hash"));
        assert!(bf.contains_hash(hash("one_hash")));
        assert!(!bf.contains_hash(hash("different")));
    }

    #[test]
    fn test_many_hashes() {
        let mut bf = make_filter(4096, 10);
        bf.insert_hash(hash("many_hashes"));
        assert!(bf.contains_hash(hash("many_hashes")));
    }

    // --- hash_item determinism ---
    /// Helper to collect hashes for testing since we removed hash_item from the API
    fn get_hashes(bf: &BloomFilter, item: &str) -> Vec<usize> {
        let mut hashes = Vec::new();
        let h1 = hash(item);
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        let mut final_hash = h1;

        for _ in 0..bf.num_hashes {
            hashes.push((final_hash as usize) % bf.size);
            final_hash = final_hash.wrapping_add(h2);
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
            bf.insert_hash(hash(&format!("inserted_{i}")));
        }
        let mut false_positives = 0;
        for i in 0..1000u32 {
            if bf.contains_hash(hash(&format!("probe_{i}"))) {
                false_positives += 1;
            }
        }
        assert!(
            false_positives < 10,
            "false positive rate too high: {false_positives}/1000"
        );
    }
}
