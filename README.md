# Bloom Filter Extended

**A Rust library providing three Bloom filter variants**: standard (bit-array), counting (deletion-capable), and scalable (auto-growing) — each with optimal parameter calculation, false-positive estimation, and set operations.

## Why It Matters

A Bloom filter is a space-efficient probabilistic data structure that tests set membership. It can produce false positives ("might be in the set") but never false negatives ("definitely not in the set"). For a 1% false-positive rate with 1 million items, a Bloom filter needs only ~1.2 MB — 50× smaller than a hash set.

Bloom filters are everywhere in production:
- **Databases** — Cassandra, PostgreSQL, and RocksDB use them to skip SSTables/disk pages that definitely don't contain a key
- **Web infrastructure** — Chrome checks malicious URLs against a Bloom filter before doing a full lookup
- **Distributed systems** — Akka uses Bloom filters for cluster membership gossip
- **Cryptocurrency** — Bitcoin uses them in SPV (Simple Payment Verification) for lightweight wallet clients

**Counting Bloom filters** solve the deletion problem: instead of 1 bit per position, use a 4-bit counter. Insertions increment, deletions decrement. This enables element removal at the cost of 4–8× more memory.

**Scalable Bloom filters** grow automatically: when the current slice fills up, a new (larger) slice is added with a tighter false-positive rate. This maintains the composite false-positive rate without knowing the set size in advance.

## How It Works

**Optimal parameters**: For n expected items and false-positive rate p, the optimal bit count is m = −n·ln(p) / (ln2)² and optimal hash count is k = (m/n)·ln2. For 1000 items at 1% FP: m ≈ 9585 bits, k ≈ 7 hashes.

**Double hashing**: Instead of k independent hash functions, this library uses the Kirsch-Mitzenmacher technique: `h_i(x) = h1(x) + i·h2(x) mod m`. Two base hashes generate all k positions, reducing computation to O(k) additions.

**Bit operations**: The standard `BloomFilter` packs bits into `u64` words. Insertion sets bit at `word[idx/64] |= 1 << (idx%64)`. Query checks `word[idx/64] & (1 << (idx%64))`. This is cache-friendly and fast.

**Set operations**: Union is bitwise OR of two filters with identical parameters. Intersection is bitwise AND. Union is exact (same FP rate); intersection may increase FP rate because OR'd bits don't distinguish which items caused them.

## Quick Start

```rust
use bloom_filter_extended::{BloomFilter, CountingBloom, ScalableBloom};

// Standard Bloom filter (1% FP rate at 1000 items)
let mut bf = BloomFilter::new(1000, 0.01);
bf.insert("hello");
bf.insert("world");
assert!(bf.might_contain("hello"));
assert!(!bf.might_contain("missing"));
println!("FP rate: {:.4}", bf.estimated_fp_rate());

// Counting Bloom filter (supports deletion)
let mut cb = CountingBloom::new(1000, 0.01);
cb.insert("temp_key");
assert!(cb.might_contain("temp_key"));
assert!(cb.remove("temp_key"));  // remove it
assert!(!cb.might_contain("temp_key"));  // gone

// Union of two filters
let mut bf1 = BloomFilter::new(100, 0.01);
let mut bf2 = BloomFilter::new(100, 0.01);
bf1.insert("a");
bf2.insert("b");
let merged = bf1.union(&bf2);
assert!(merged.might_contain("a"));
assert!(merged.might_contain("b"));
```

## API

- **`BloomFilter`** — Bit-array filter: `insert()`, `might_contain()`, `union()`, `intersection()`, `memory_bytes()`, `estimated_fp_rate()`
- **`CountingBloom`** — Counter-array filter: `insert()`, `remove()`, `might_contain()`, `merge()`
- **`ScalableBloom`** — Auto-growing filter chain: `insert()`, `might_contain()`, `num_filters()`

## Architecture Notes

Provides the set-membership primitives for SuperInstance data pipelines. The counting variant is used in fleet deduplication; the scalable variant in unbounded event streams where the cardinality isn't known upfront. See the [architecture overview](https://github.com/SuperInstance/SuperInstance/blob/main/ARCHITECTURE.md).

## License

MIT
