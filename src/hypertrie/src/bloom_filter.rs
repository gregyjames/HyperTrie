use std::hash::Hasher;
use bit_vec::BitVec;
use gxhash::GxHasher;

const SEED: i64 = 1846279233212321312;

pub struct BloomFilter {
    bit_array: BitVec,
    size: usize,
    num_hashes: usize,
}

impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let bit_array = BitVec::from_elem(size, false);
        BloomFilter {
            bit_array,
            size,
            num_hashes,
        }
    }

    pub fn insert(&mut self, item: &str) {
        let hashes = self.hash_item(item);
        for hash in hashes {
            self.bit_array.set(hash % self.size, true);
        }
    }

    pub fn contains(&self, item: &str) -> bool {
        let hashes = self.hash_item(item);
        for hash in hashes {
            if !self.bit_array[hash % self.size] {
                return false; // Definitely not in the set
            }
        }
        true // Maybe in the set (false positives possible)
    }

    fn hash_item(&self, item: &str) -> Vec<usize> {
        let mut hashes = Vec::new();
        
        for i in 0..self.num_hashes {
            let mut hasher = GxHasher::with_seed(SEED);
            let input = item.as_bytes();
            
            hasher.write(input);
            hasher.write(&[i as u8]); // Adding a unique salt to make the hashes different

            let hash = hasher.finish() as usize;
            hashes.push(hash);
        }
        hashes
    }
} 