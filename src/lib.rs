//! Extended Bloom filter with counting, union, intersection, and scalability.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// A counting Bloom filter that supports deletion.
#[derive(Debug, Clone)]
pub struct CountingBloom {
    counters: Vec<u32>,
    num_hashes: usize,
    count: usize,
}

impl CountingBloom {
    pub fn new(expected_items: usize, fp_rate: f64) -> Self {
        let (num_bits, num_hashes) = Self::optimal_params(expected_items, fp_rate);
        Self {
            counters: vec![0; num_bits],
            num_hashes,
            count: 0,
        }
    }

    pub fn with_params(num_bits: usize, num_hashes: usize) -> Self {
        Self {
            counters: vec![0; num_bits],
            num_hashes,
            count: 0,
        }
    }

    fn optimal_params(n: usize, p: f64) -> (usize, usize) {
        let ln2 = std::f64::consts::LN_2;
        let m = (-((n as f64) * p.ln()) / (ln2 * ln2)).ceil() as usize;
        let k = ((m as f64) / (n as f64) * ln2).ceil() as usize;
        (m.max(64), k.max(1))
    }

    fn hash_indices<T: Hash + ?Sized>(&self, item: &T, seed: usize) -> Vec<usize> {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        (seed as u64).hash(&mut hasher);
        let h1 = hasher.finish();

        let mut hasher2 = DefaultHasher::new();
        (seed as u64).wrapping_add(0x9e3779b97f4a7c15).hash(&mut hasher2);
        item.hash(&mut hasher2);
        let h2 = hasher2.finish();

        (0..self.num_hashes)
            .map(|i| {
                let h = h1.wrapping_add((i as u64).wrapping_mul(h2));
                (h % (self.counters.len() as u64)) as usize
            })
            .collect()
    }

    /// Insert an item.
    pub fn insert<T: Hash + ?Sized>(&mut self, item: &T) {
        for idx in self.hash_indices(item, 0) {
            self.counters[idx] = self.counters[idx].saturating_add(1);
        }
        self.count += 1;
    }

    /// Remove an item. Returns false if the item was likely not present.
    pub fn remove<T: Hash + ?Sized>(&mut self, item: &T) -> bool {
        let indices = self.hash_indices(item, 0);
        // Check all counters > 0 first
        if indices.iter().any(|&i| self.counters[i] == 0) {
            return false;
        }
        for idx in &indices {
            self.counters[*idx] = self.counters[*idx].saturating_sub(1);
        }
        self.count = self.count.saturating_sub(1);
        true
    }

    /// Check if an item might be in the set.
    pub fn might_contain<T: Hash + ?Sized>(&self, item: &T) -> bool {
        self.hash_indices(item, 0).iter().all(|&i| self.counters[i] > 0)
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Estimated false positive rate at current fill level.
    pub fn estimated_fp_rate(&self) -> f64 {
        let m = self.counters.len() as f64;
        let k = self.num_hashes as f64;
        let n = self.count.max(1) as f64;
        (1.0 - (-k * n / m).exp()).powf(k)
    }

    /// Merge another counting Bloom filter into this one (counter-wise max).
    pub fn merge(&mut self, other: &CountingBloom) {
        assert_eq!(self.counters.len(), other.counters.len());
        assert_eq!(self.num_hashes, other.num_hashes);
        for i in 0..self.counters.len() {
            self.counters[i] = self.counters[i].max(other.counters[i]);
        }
        self.count += other.count;
    }
}

/// Standard Bloom filter (bit-based, memory-efficient).
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<u64>,
    num_bits: usize,
    num_hashes: usize,
    count: usize,
}

impl BloomFilter {
    pub fn new(expected_items: usize, fp_rate: f64) -> Self {
        let (num_bits, num_hashes) = CountingBloom::optimal_params(expected_items, fp_rate);
        let words = (num_bits + 63) / 64;
        Self {
            bits: vec![0u64; words],
            num_bits,
            num_hashes,
            count: 0,
        }
    }

    fn hash_indices<T: Hash + ?Sized>(&self, item: &T) -> Vec<usize> {
        let mut hasher = DefaultHasher::new();
        item.hash(&mut hasher);
        let h1 = hasher.finish();

        let mut hasher2 = DefaultHasher::new();
        0x9e3779b97f4a7c15u64.hash(&mut hasher2);
        item.hash(&mut hasher2);
        let h2 = hasher2.finish();

        (0..self.num_hashes)
            .map(|i| {
                let h = h1.wrapping_add((i as u64).wrapping_mul(h2));
                (h % (self.num_bits as u64)) as usize
            })
            .collect()
    }

    pub fn insert<T: Hash + ?Sized>(&mut self, item: &T) {
        for idx in self.hash_indices(item) {
            let word = idx / 64;
            let bit = idx % 64;
            self.bits[word] |= 1u64 << bit;
        }
        self.count += 1;
    }

    pub fn might_contain<T: Hash + ?Sized>(&self, item: &T) -> bool {
        for idx in self.hash_indices(item) {
            let word = idx / 64;
            let bit = idx % 64;
            if self.bits[word] & (1u64 << bit) == 0 {
                return false;
            }
        }
        true
    }

    pub fn count(&self) -> usize {
        self.count
    }

    /// Compute the union of two Bloom filters (same params required).
    pub fn union(&self, other: &BloomFilter) -> BloomFilter {
        assert_eq!(self.num_bits, other.num_bits);
        assert_eq!(self.num_hashes, other.num_hashes);
        let bits: Vec<u64> = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a | b).collect();
        BloomFilter {
            bits,
            num_bits: self.num_bits,
            num_hashes: self.num_hashes,
            count: self.count + other.count,
        }
    }

    /// Compute the intersection of two Bloom filters.
    pub fn intersection(&self, other: &BloomFilter) -> BloomFilter {
        assert_eq!(self.num_bits, other.num_bits);
        assert_eq!(self.num_hashes, other.num_hashes);
        let bits: Vec<u64> = self.bits.iter().zip(&other.bits).map(|(&a, &b)| a & b).collect();
        BloomFilter {
            bits,
            num_bits: self.num_bits,
            num_hashes: self.num_hashes,
            count: 0, // can't accurately estimate intersection count from bits
        }
    }

    /// Memory usage in bytes.
    pub fn memory_bytes(&self) -> usize {
        self.bits.len() * 8
    }

    /// Estimated false positive rate.
    pub fn estimated_fp_rate(&self) -> f64 {
        let m = self.num_bits as f64;
        let k = self.num_hashes as f64;
        let n = self.count.max(1) as f64;
        (1.0 - (-k * n / m).exp()).powf(k)
    }
}

/// A scalable Bloom filter that grows as needed to maintain a target FP rate.
#[derive(Debug, Clone)]
pub struct ScalableBloom {
    filters: Vec<BloomFilter>,
    initial_capacity: usize,
    fp_rate: f64,
    growth_factor: usize,
    tightening_ratio: f64,
    count: usize,
}

impl ScalableBloom {
    pub fn new(initial_capacity: usize, fp_rate: f64) -> Self {
        let mut sb = Self {
            filters: Vec::new(),
            initial_capacity,
            fp_rate,
            growth_factor: 2,
            tightening_ratio: 0.85,
            count: 0,
        };
        sb.add_filter();
        sb
    }

    fn add_filter(&mut self) {
        let capacity = self.initial_capacity * self.growth_factor.pow(self.filters.len() as u32);
        let fp = self.fp_rate * self.tightening_ratio.powi(self.filters.len() as i32);
        self.filters.push(BloomFilter::new(capacity, fp));
    }

    pub fn insert<T: Hash + ?Sized>(&mut self, item: &T) {
        // Insert into the last filter; if it's getting full, add a new one
        let last = self.filters.last_mut().unwrap();
        last.insert(item);
        self.count += 1;
        // Rough heuristic: add new filter when count exceeds capacity
        if last.count > self.initial_capacity * self.growth_factor.pow((self.filters.len() - 1) as u32) {
            self.add_filter();
        }
    }

    pub fn might_contain<T: Hash + ?Sized>(&self, item: &T) -> bool {
        self.filters.iter().any(|f| f.might_contain(item))
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn num_filters(&self) -> usize {
        self.filters.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_bloom() {
        let mut bf: BloomFilter = BloomFilter::new(100, 0.01);
        bf.insert("hello");
        bf.insert("world");
        assert!(bf.might_contain("hello"));
        assert!(bf.might_contain("world"));
        assert!(!bf.might_contain("missing"));
    }

    #[test]
    fn test_counting_bloom_remove() {
        let mut cb = CountingBloom::new(100, 0.01);
        cb.insert("hello");
        assert!(cb.might_contain("hello"));
        assert!(cb.remove("hello"));
        assert!(!cb.might_contain("hello"));
    }

    #[test]
    fn test_bloom_union() {
        let mut bf1 = BloomFilter::new(100, 0.01);
        let mut bf2 = BloomFilter::new(100, 0.01);
        bf1.insert("a");
        bf2.insert("b");
        let union = bf1.union(&bf2);
        assert!(union.might_contain("a"));
        assert!(union.might_contain("b"));
    }

    #[test]
    fn test_scalable_bloom() {
        let mut sb = ScalableBloom::new(10, 0.01);
        for i in 0..100 {
            sb.insert(&i);
        }
        assert!(sb.might_contain(&50));
        assert!(!sb.might_contain(&200));
        assert!(sb.num_filters() > 1);
    }
}

/// FNV-1a 64 — the digest every substrate in the SuperInstance fleet agrees on.
pub const FNV_OFFSET: u64 = 0xcbf29ce484222325;
pub const FNV_PRIME: u64 = 0x100000001b3;

#[inline]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in bytes {
        h = (h ^ b as u64).wrapping_mul(FNV_PRIME);
    }
    h
}

/// True if this crate's FNV-1a still agrees with the rest of the fleet.
pub fn canary_holds() -> bool {
    fnv1a64("café Δ 日本語".as_bytes()) == 0x024a555471370b18d
}
