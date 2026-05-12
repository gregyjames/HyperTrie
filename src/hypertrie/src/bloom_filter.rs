use gxhash::gxhash64;

const SEED: i64 = 1846279233212321312;

pub struct BloomFilter {
    bit_array: Box<[u64]>,
    size_mask: usize,
    num_hashes: usize,
}

impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let optimized_size = size.max(64).next_power_of_two();
        let u64_count = optimized_size >> 6;
        BloomFilter {
            bit_array: vec![0u64; u64_count].into_boxed_slice(),
            size_mask: optimized_size - 1,
            num_hashes,
        }
    }

    #[inline(always)]
    pub fn insert(&mut self, item: &str) {
        let h = gxhash64(item.as_bytes(), SEED);
        let mut h1 = h as usize;
        let h2 = (h >> 32) as usize;

        for _ in 0..self.num_hashes {
            let index = h1 & self.size_mask;
            unsafe {
                *self.bit_array.get_unchecked_mut(index >> 6) |= 1 << (index & 63);
            }
            h1 = h1.wrapping_add(h2);
        }
    }

    #[inline(always)]
    pub fn contains(&self, item: &str) -> bool {
        let h = gxhash64(item.as_bytes(), SEED);
        let mut h1 = h as usize;
        let h2 = (h >> 32) as usize;

        for _ in 0..self.num_hashes {
            let index = h1 & self.size_mask;
            unsafe {
                if (*self.bit_array.get_unchecked(index >> 6) & (1 << (index & 63))) == 0 {
                    return false;
                }
            }
            h1 = h1.wrapping_add(h2);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_filter(size: usize, num_hashes: usize) -> BloomFilter {
        BloomFilter::new(size, num_hashes)
    }

    #[test]
    fn test_new_contains_nothing() {
        let bf = make_filter(1024, 3);
        assert!(!bf.contains("hello"));
        assert!(!bf.contains("world"));
    }

    #[test]
    fn test_new_size_one() {
        let mut bf = make_filter(1, 1);
        bf.insert("x");
        assert!(bf.contains("x"));
    }

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
        assert!(!bf.contains("hello"));
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

    fn get_hashes(bf: &BloomFilter, item: &str) -> Vec<usize> {
        let mut hashes = Vec::new();
        let h = gxhash64(item.as_bytes(), SEED);
        let mut h1 = h as usize;
        let h2 = (h >> 32) as usize;

        for _ in 0..bf.num_hashes {
            let index = h1 & bf.size_mask;
            hashes.push(index);
            h1 = h1.wrapping_add(h2);
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
        let bf = make_filter(1024, 4);
        let hashes = get_hashes(&bf, "salt_test");
        let unique: std::collections::HashSet<usize> = hashes.iter().copied().collect();
        assert!(unique.len() > 1, "expected distinct hash values per slot");
    }

    #[test]
    fn test_hash_count_matches_num_hashes() {
        let bf = make_filter(1024, 5);
        let hashes = get_hashes(&bf, "count");
        assert_eq!(hashes.len(), 5, "Should generate exactly num_hashes values");
    }

    #[test]
    fn test_false_positive_rate_is_reasonable() {
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
