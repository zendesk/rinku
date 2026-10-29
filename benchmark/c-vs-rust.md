# Why the Rust rewrite: historical performance

The Rust rewrite was faster on the measured workloads. It also moves the
production scanner and renderer into a Rust core that forbids unsafe code. In
the 2026-10-19 comparison, general workloads used **22% less time** than C.
Mail text-node workloads used **18% less time**. These percentages use the
equal-weight geometric mean of Rust/C time: 0.779 for general workloads and
0.823 for mail workloads. Repeated invalid email candidates changed from
quadratic to near-linear scaling.

These are historical measurements. They are not a maintained baseline or a
release gate. The working tree does not keep raw samples or intermediate
reports. Git history keeps the original reports under `docs/benchmarks/`.

## Measurement context

We ran the measurements on Ruby 3.3.12 and arm64 macOS. Both extensions used
release builds. YJIT was disabled and the garbage collector was enabled. The
historical C revision was
`67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863`.

We ran six independent C/Rust process pairs in alternating order. Each case
used a 100 ms sample and a 50 ms warmup. No build or validation job ran at the
same time. We checked output before timing. All 1,061 differential
observations matched. Table values are arithmetic means.

All 32 measured cases had an upper paired-bootstrap 95% confidence bound below
1.0 for Rust/C time. These workstation measurements do not predict every
platform. They are not controlled release evidence. We did not collect native
allocation profiles. Ruby object counts do not measure native memory.

## Supplied workloads

General cases time the full URL-then-email operation. Mail cases time URL-only
linking of text nodes that were already extracted. The measurements exclude
HTML extraction. See [the workload definitions](README.md#workloads).

| General workload | C ns/op | Rust ns/op |
|---|---:|---:|
| Plain text | 367.8 | 196.9 |
| Punctuated URLs | 816.9 | 670.7 |
| Short domain and email | 703.8 | 633.4 |
| Existing anchor and unlinked URL | 723.3 | 554.2 |
| Skip code and pre | 799.2 | 648.9 |
| Escaped angle brackets | 576.2 | 500.6 |
| Balanced parentheses and punctuation | 921.3 | 702.0 |
| Unicode multiline email | 797.6 | 686.6 |
| HTML email mixed links | 1,462.6 | 1,283.1 |
| Ticket reference is not a link | 721.9 | 512.8 |
| Long mixed email | 2,510.3 | 1,847.8 |

| Mail workload | C ns/op | Rust ns/op |
|---|---:|---:|
| HTTP, HTTPS, FTP URLs | 928.4 | 741.5 |
| www and protocol URL in the same node | 635.4 | 514.8 |
| Email address is not linked | 479.1 | 383.6 |
| Escaped query from serialized text node | 535.4 | 449.9 |
| Parenthesized URL and punctuation | 535.5 | 467.5 |
| Unicode text node | 454.3 | 413.4 |
| Long mixed email text node | 1,556.7 | 1,152.2 |

## Supplemental workloads

| Workload | C µs/op | Rust µs/op |
|---|---:|---:|
| No links (13,312 bytes) | 5.487 | 0.923 |
| Large mixed input | 866.567 | 659.072 |
| Reverse callbacks (64 links) | 17.792 | 17.294 |

After the callback fixes, the reverse-callback Rust/C ratio was
**0.972 (95% CI 0.953–0.993)**. This replaces the earlier exploratory result of
1.094 (1.074–1.116), which showed a regression.

Identity callbacks on `'http://a.com ' * links`. Values are milliseconds per
call:

| Links | Mutable C | Mutable Rust | Frozen C | Frozen Rust |
|---:|---:|---:|---:|---:|
| 1,000 | 0.173 | 0.160 | 0.174 | 0.156 |
| 4,000 | 0.662 | 0.619 | 0.656 | 0.617 |
| 16,000 | 2.629 | 2.524 | 2.656 | 2.414 |
| 64,000 | 10.776 | 9.864 | 10.987 | 9.697 |

Both source types scale approximately linearly. The next table shows repeated
invalid email candidates (`'a@' * repetitions`) with no callback. Values are
milliseconds per call:

| Repetitions | C | Rust |
|---:|---:|---:|
| 1,000 | 0.788 | 0.021 |
| 4,000 | 12.086 | 0.085 |
| 16,000 | 190.806 | 0.341 |

At 16,000 repetitions, Rust was about **560 times faster**. It avoids the
quadratic candidate rejection in C. This adversarial case does not represent
typical mail traffic. It is not part of the common-workload result above.

## Always-owned input comparison (2026-10-28)

This experiment changed the no-block binding to copy source and attribute bytes
into Rust-owned buffers before parsing. We compared it with the borrowing Rust build
at `2888312d26cacf27e6643a2443df3eea4d141d9f` and a fresh build of the same
historical C revision listed above.

The comparison used Ruby 3.3.12 on arm64 macOS, Rust 1.99.1 release builds,
YJIT disabled, and GC enabled. We ran the existing `benchmark/worker.rb` in
six rounds, using each permutation of the three implementations once. Each
worker checked outputs before timing all 38 workloads, with a 200 ms target
sample and 100 ms warmup. No builds or tests ran alongside timing.

Common-workload ratios are equal-weight geometric means of per-case mean
times. Confidence intervals use 10,000 paired bootstrap resamples of the six
rounds (seed 20261028).

| Workloads | Copy / borrow time (95% CI) | Copy / C time (95% CI) |
|---|---:|---:|
| General (11 cases) | 1.115 (1.067–1.163) | 0.830 (0.794–0.864) |
| Mail (7 cases) | 1.070 (1.027–1.113) | 0.853 (0.830–0.876) |

Copying adds about **11.5%** to general-workload time and **7.0%** to mail time,
while still using **17.0%** and **14.7% less time than C**, respectively.
All 38 cases had lower mean times than C, with individual upper paired-bootstrap
95% confidence bounds below 1.0. Selected arithmetic means:

| Workload | Borrowing Rust µs/op | Copying Rust µs/op | C µs/op |
|---|---:|---:|---:|
| Plain text, two passes | 0.190 | 0.254 | 0.378 |
| No links (13,312 bytes) | 0.904 | 1.107 | 5.318 |
| Large mixed input | 632.870 | 587.517 | 837.919 |
| Reverse callbacks (64 links) | 16.527 | 16.394 | 18.072 |

The small no-link case shows the largest relative copying cost. The large-input
speedup is an observed result, not a general guarantee that copying is faster.
Callback inputs already used owned snapshots.

Ruby allocations per operation matched between the two Rust builds. The new
native buffers additionally hold the source and attribute bytes; this experiment
did not profile native peak memory. Local raw samples, library hashes, summaries,
and confidence intervals are in the ignored `benchmark/results/copying/` directory.

The copying build passed 73 Ruby tests, 11 GC-stress tests, 11 Rust tests, formatting,
Clippy, and all 1,075 C differential observations. We kept copying for its smaller
unsafe borrowing scope while retaining the measured advantage over C.
