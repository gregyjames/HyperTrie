use gxhash::GxHasher;
use std::hash::Hasher;

const SEED: i64 = 1846279233212321312;

pub struct BloomFilter {
    bit_array: Vec<u64>,
    size: usize,
    num_hashes: usize,
}

impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let u64_count = (size + 63) / 64;
        BloomFilter {
            bit_array: vec![0; u64_count],
            size,
            num_hashes,
        }
    }

    pub fn insert(&mut self, item: &str) {
        let h1 = self.get_base_hash(item);
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        let size_mask = self.size - 1;
        for i in 0..self.num_hashes {
            let final_hash = h1.wrapping_add((i as u64).wrapping_mul(h2));
            let index = (final_hash as usize) & size_mask;

            let word_idx = index >> 6;
            let bit_idx = index & 63;
            unsafe {
                *self.bit_array.get_unchecked_mut(word_idx) |= 1 << bit_idx;
            }
        }
    }

    pub fn contains(&self, item: &str) -> bool {
        let h1 = self.get_base_hash(item);
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        let size_mask = self.size - 1;
        for i in 0..self.num_hashes {
            let final_hash = h1.wrapping_add((i as u64).wrapping_mul(h2));
            let index = (final_hash as usize) & size_mask;

            let word_idx = index >> 6;
            let bit_idx = index & 63;
            unsafe {
                if (*self.bit_array.get_unchecked(word_idx) & (1 << bit_idx)) == 0 {
                    return false;
                }
            }
        }
        true // Maybe in the set (false positives possible)
    }

    #[inline(always)]
    fn get_base_hash(&self, item: &str) -> u64 {
        let mut hasher = GxHasher::with_seed(SEED);
        hasher.write(item.as_bytes());
        hasher.finish()
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
        assert!(!bf.contains("hello"));
        assert!(!bf.contains("world"));
    }

    #[test]
    fn test_new_size_one() {
        // degenerate but shouldn't panic
        let mut bf = make_filter(1, 1);
        bf.insert("x");
        assert!(bf.contains("x"));
    }

    // --- insert / contains ---

    #[test]
    fn test_inserted_item_is_found() {
        let mut bf = make_filter(1024, 3);
        bf.insert("hello");
        assert!(bf.contains("hello"));
    }

    #[test]
    fn test_multiple_insertions() {
        let mut bf = make_filter(1024, 3);
        let words = ["apple", "banana", "cherry", "date", "elderberry"];
        for w in &words {
            bf.insert(w);
        }
        for w in &words {
            assert!(bf.contains(w), "expected '{w}' to be found");
        }
    }

    #[test]
    fn test_non_inserted_item_likely_absent() {
        // With a large filter and few items, false positives should not occur
        // for these specific values — if this ever flakes, increase size.
        let mut bf = make_filter(8192, 4);
        bf.insert("present");
        assert!(!bf.contains("absent"));
        assert!(!bf.contains("also_absent"));
    }

    #[test]
    fn test_insert_empty_string() {
        let mut bf = make_filter(1024, 3);
        bf.insert("");
        assert!(bf.contains(""));
    }

    #[test]
    fn test_empty_string_not_present_by_default() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains(""));
    }

    #[test]
    fn test_insert_is_idempotent() {
        let mut bf = make_filter(1024, 3);
        bf.insert("repeat");
        bf.insert("repeat");
        assert!(bf.contains("repeat"));
    }

    #[test]
    fn test_unicode_item() {
        let mut bf = make_filter(1024, 3);
        bf.insert("héllo");
        bf.insert("日本語");
        assert!(bf.contains("héllo"));
        assert!(bf.contains("日本語"));
        assert!(!bf.contains("hello")); // ASCII variant is distinct
    }

    #[test]
    fn test_case_sensitive() {
        let mut bf = make_filter(1024, 3);
        bf.insert("Hello");
        assert!(bf.contains("Hello"));
        assert!(!bf.contains("hello"));
        assert!(!bf.contains("HELLO"));
    }

    #[test]
    fn test_similar_strings_are_distinct() {
        let mut bf = make_filter(8192, 4);
        bf.insert("abc");
        assert!(!bf.contains("ab"));
        assert!(!bf.contains("abcd"));
        assert!(!bf.contains("ABC"));
    }

    // --- num_hashes boundary ---

    #[test]
    fn test_single_hash() {
        let mut bf = make_filter(1024, 1);
        bf.insert("one_hash");
        assert!(bf.contains("one_hash"));
        assert!(!bf.contains("different"));
    }

    #[test]
    fn test_many_hashes() {
        let mut bf = make_filter(4096, 10);
        bf.insert("many_hashes");
        assert!(bf.contains("many_hashes"));
    }

    // --- hash_item determinism ---
    /// Helper to collect hashes for testing since we removed hash_item from the API
    fn get_hashes(bf: &BloomFilter, item: &str) -> Vec<usize> {
        let mut hashes = Vec::new();
        let h1 = bf.get_base_hash(item);
        let h2 = h1.wrapping_mul(0x9e3779b97f4a7c15);
        for i in 0..bf.num_hashes {
            let final_hash = h1.wrapping_add((i as u64).wrapping_mul(h2));
            hashes.push((final_hash as usize) % bf.size);
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
            bf.insert(&format!("inserted_{i}"));
        }
        let mut false_positives = 0;
        for i in 0..1000u32 {
            if bf.contains(&format!("probe_{i}")) {
                false_positives += 1;
            }
        }
        assert!(
            false_positives < 10,
            "false positive rate too high: {false_positives}/1000"
        );
    }
}
