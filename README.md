# Bloom Filter Extended

**A Rust library providing three Bloom filter variants**: standard (bit-array), counting (deletion-capable), and scalable (auto-growing) — each with optimal parameter calculation, false-positive estimation, and set operations.

## Why It Matters

A Bloom filter is a space-efficient probabilistic data structure that tests set membership. It can produce **false positives** ("might be in the set") but **never false negatives** ("definitely not in the set"). For a 1% false-positive rate with 1 million items, a Bloom filter needs only ~1.2 MB — 50× smaller than a hash set storing the same data.

Bloom filters are everywhere in production systems:

- **Databases** — Cassandra, PostgreSQL (Bloom index), and RocksDB use them to skip SSTables and disk pages that definitely don't contain a key, avoiding expensive disk reads
- **Web browsers** — Chrome checks URLs against a Bloom filter of malicious sites before doing a full Safe Browsing lookup
- **Distributed systems** — Akka and Cassandra use Bloom filters for cluster membership gossip and hinted handoff
- **Cryptocurrency** — Bitcoin SPV (Simple Payment Verification) wallets use Bloom filters to request relevant transactions from full nodes
- **CDNs** — Content Delivery Networks use them to check if a resource is cached at an edge node

**Counting Bloom filters** solve the deletion problem: instead of 1 bit per position, use a 4-bit counter. Insertions increment, deletions decrement. This enables element removal at the cost of 4–8× more memory. Without counting, you cannot remove elements from a standard Bloom filter.

**Scalable Bloom filters** (Almeida et al., 2007) grow automatically: when the current slice fills up, a new (larger) slice is added with a tighter false-positive rate. This maintains the composite false-positive rate without knowing the set size in advance — critical for streaming data with unbounded cardinality.

## How It Works

### Optimal Parameter Calculation

For n expected items and target false-positive rate p, the optimal parameters are:

```
m = −n · ln(p) / (ln2)²         (number of bits)
k = (m / n) · ln2               (number of hash functions)
```

Derived by Bloom (1970) and refined by Mitzenmacher & Upfal (2005). For 1,000 items at 1% FP rate: m ≈ 9,585 bits (~1.2 KB), k ≈ 7 hashes.

**Memory savings vs. hash set:** For 1M items at 1% FP:
- Bloom: m = 9.585M bits = **1.2 MB**
- Hash set (64-bit hashes): 8MB + 50% load factor overhead ≈ **12 MB**
- **10× smaller**

### Double Hashing (Kirsch-Mitzenmacher)

Instead of k independent hash functions, this library uses the **Kirsch-Mitzenmacher** technique (2006):

```
h_i(x) = (h₁(x) + i · h₂(x)) mod m    for i = 0, 1, ..., k-1
```

Two base hashes (`h₁`, `h₂`) generate all k positions. This reduces computation from O(k) hash evaluations to O(2), with O(k) additions — a 3.5× speedup for k = 7.

**Complexity per operation:**

| Operation | Time | Space |
|-----------|------|-------|
| Insert | O(k) | O(1) |
| Lookup | O(k) | O(1) |
| Remove (counting only) | O(k) | O(1) |
| Union | O(m/64) word-level OR | O(m) |
| Intersection | O(m/64) word-level AND | O(m) |

### False-Positive Rate

After n insertions into m bits with k hashes, the FP rate is:

```
p ≈ (1 − e^(−kn/m))^k
```

The `estimated_fp_rate()` method computes this on-the-fly. The optimal k minimizes p, yielding the formula above.

### Set Operations

**Union** (bitwise OR) of two filters with identical parameters:

```
result[i] = a[i] | b[i]
```

The union of two Bloom filters represents the union of their sets. The FP rate increases because more bits are set.

**Intersection** (bitwise AND):

```
result[i] = a[i] & b[i]
```

The intersection's FP rate is harder to bound — it may be higher or lower depending on overlap.

**Merge** (counting filters): counter-wise max:

```
result[i] = max(a[i], b[i])
```

### Scalable Bloom Filter

A scalable Bloom filter is a sequence of slices, each with geometrically increasing capacity and geometrically decreasing FP rate:

```
slice_i: capacity = C₀ · r^i,  FP_rate = p₀ · s^i
```

Where r is the growth ratio (default 2) and s is the tightening ratio (default 0.85). The composite FP rate converges:

```
P_total = 1 − Π(1 − pᵢ) ≤ p₀ / (1 − s)
```

For p₀ = 0.01 and s = 0.85: P_total ≤ 0.01 / 0.15 ≈ 6.67%. This is bounded by design.

**Lookup** checks all slices (any match → positive):

```
might_contain(x) = ∃i: slice_i.might_contain(x)
```

This is O(num_slices × k) per lookup.

## Quick Start

```rust
use bloom_filter_extended::{BloomFilter, CountingBloom, ScalableBloom};

// Standard Bloom filter (1% FP rate at 1000 items)
let mut bf = BloomFilter::new(1000, 0.01);
bf.insert("hello");
bf.insert("world");
assert!(bf.might_contain("hello"));
assert!(!bf.might_contain("missing"));  // probably
println!("FP rate: {:.4}", bf.estimated_fp_rate());

// Counting Bloom filter (supports deletion)
let mut cb = CountingBloom::new(1000, 0.01);
cb.insert("temp_key");
assert!(cb.might_contain("temp_key"));
assert!(cb.remove("temp_key"));
assert!(!cb.might_contain("temp_key"));

// Union of two filters
let mut bf1 = BloomFilter::new(100, 0.01);
let mut bf2 = BloomFilter::new(100, 0.01);
bf1.insert("a");
bf2.insert("b");
let merged = bf1.union(&bf2);
assert!(merged.might_contain("a"));
assert!(merged.might_contain("b"));

// Scalable Bloom filter for unbounded streams
let mut sb = ScalableBloom::new(10, 0.01);
for i in 0..100 {
    sb.insert(&i);
}
assert!(sb.might_contain(&50));
assert!(sb.num_filters() > 1);  // auto-grew
```

## API

### `BloomFilter` (bit-array, memory-efficient)

| Method | Description |
|--------|-------------|
| `new(expected_items, fp_rate)` | Create with optimal params |
| `insert(item)` | Add item to set |
| `might_contain(item) → bool` | Probabilistic membership test |
| `union(&other) → BloomFilter` | Set union (bitwise OR) |
| `intersection(&other) → BloomFilter` | Set intersection (bitwise AND) |
| `memory_bytes() → usize` | Memory usage |
| `estimated_fp_rate() → f64` | Current false-positive rate |

### `CountingBloom` (4-bit counters, supports deletion)

| Method | Description |
|--------|-------------|
| `new(expected_items, fp_rate)` | Create with optimal params |
| `insert(item)` | Increment counters |
| `remove(item) → bool` | Decrement counters (false if absent) |
| `might_contain(item) → bool` | Probabilistic membership test |
| `merge(&other)` | Counter-wise max merge |

### `ScalableBloom` (auto-growing chain)

| Method | Description |
|--------|-------------|
| `new(initial_capacity, fp_rate)` | Create first slice |
| `insert(item)` | Insert, auto-grow if needed |
| `might_contain(item) → bool` | Check all slices |
| `num_filters() → usize` | Current slice count |

## Architecture Notes

Provides the set-membership primitives for SuperInstance data pipelines. The counting variant is used in fleet deduplication; the scalable variant in unbounded event streams where the cardinality isn't known upfront.

Within γ + η = C, a Bloom filter instantiates the conservation law as the **no-false-negative invariant**: information about set membership is conserved (C). Insertions add energy (γ), queries test energy (η), and the invariant guarantees that any inserted item will always be found. False positives are the "thermal noise" — the price of compression — but the signal (true members) is never lost.

See the [architecture overview](https://github.com/casey-digennaro/bloom-filter-extended/blob/main/ARCHITECTURE.md).

## References

1. Bloom, B.H. (1970). "Space/Time Trade-offs in Hash Coding with Allowable Errors." *Communications of the ACM*, 13(7), 422–426.
2. Almeida, P.S., Baquero, C., Preguiça, N., & Hutchison, D. (2007). "Scalable Bloom Filters." *Information Processing Letters*, 101(6), 255–261.
3. Kirsch, A. & Mitzenmacher, M. (2006). "Less Hashing, Same Performance: Building a Better Bloom Filter." *ESA 2006*, 456–467.
4. Mitzenmacher, M. & Upfal, E. (2005). *Probability and Computing*. Cambridge University Press. Chapter 5: "Bloom Filters."
5. Fan, B. et al. (2000). "Summary Cache: A Scalable Wide-Area Web Cache Sharing Protocol." *IEEE/ACM TON*, 8(3), 281–293. (Counting Bloom filters)

## License

MIT
