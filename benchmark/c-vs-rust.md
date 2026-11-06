# Historical performance of the Rust rewrite

The Rust rewrite was faster on the measured workloads. It moved the production
scanner and renderer into a Rust core that forbids unsafe code. In the
2026-10-19 comparison, general workloads took **22% less time** than C. Mail
text-node workloads took **18% less time**. These figures use the equal-weight
geometric mean of Rust/C time: 0.779 for general workloads and 0.823 for mail
workloads. Repeated invalid email candidates now scale near-linearly instead of
quadratically.

These measurements are historical. They are not a maintained baseline or a
release gate. The working tree does not contain raw samples or intermediate
reports. Git history contains the original reports in `docs/benchmarks/`.

## Measurement context

We used Ruby 3.3.12 on arm64 macOS. We built both extensions in release mode.
We disabled YJIT and enabled the garbage collector. The historical C revision
was `67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863`.

We ran six independent C/Rust process pairs and alternated their order. Each
case used a 100 ms sample and a 50 ms warmup. No build or validation job ran
during a measurement. We checked output before timing. All 1,061 differential
observations matched. Table values show arithmetic means.

For all 32 cases, the upper paired-bootstrap 95% confidence bound was below
1.0 for the Rust/C time ratio. These workstation measurements do not predict
performance on every platform. They are not controlled release evidence. We
did not collect native allocation profiles. Ruby object counts do not measure
native memory.

## Supplied workloads

General cases run URL linking, then email linking. Mail cases run URL linking
on text nodes that were already extracted. The measurements do not include HTML
extraction. See [the workload definitions](README.md#workloads).

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

After we fixed callback handling, the reverse-callback Rust/C ratio was
**0.972 (95% CI 0.953–0.993)**. This replaces an earlier exploratory result of
1.094 (1.074–1.116), which showed a regression.

Identity-callback workload: `'http://a.com ' * links`. Values are milliseconds
per call:

| Links | Mutable C | Mutable Rust | Frozen C | Frozen Rust |
|---:|---:|---:|---:|---:|
| 1,000 | 0.173 | 0.160 | 0.174 | 0.156 |
| 4,000 | 0.662 | 0.619 | 0.656 | 0.617 |
| 16,000 | 2.629 | 2.524 | 2.656 | 2.414 |
| 64,000 | 10.776 | 9.864 | 10.987 | 9.697 |

Mutable and frozen source strings scale close to linearly. The next table shows
repeated invalid email candidates, `'a@' * repetitions`, with no callback.
Values are milliseconds per call:

| Repetitions | C | Rust |
|---:|---:|---:|
| 1,000 | 0.788 | 0.021 |
| 4,000 | 12.086 | 0.085 |
| 16,000 | 190.806 | 0.341 |

At 16,000 repetitions, Rust was about **560 times faster** than C. The Rust
parser avoids C's quadratic candidate rejection. This case uses unusual input.
It does not represent typical mail traffic or the common-workload results above.

## Always-owned input comparison (2026-10-28)

In this experiment, the no-callback binding copied source and attribute bytes
into Rust-owned buffers before parsing. We compared this build with the
borrowing Rust build at `2888312d26cacf27e6643a2443df3eea4d141d9f`. We also
built the historical C revision listed above.

We used Ruby 3.3.12 on arm64 macOS. We built the Rust versions in release mode
with Rust 1.99.1. We disabled YJIT and enabled GC. We ran
`benchmark/worker.rb` for six rounds. Each round tested all six possible orders
of the three implementations. Each worker checked outputs before timing all 38
workloads. The target sample was 200 ms, with a 100 ms warmup. No build or test
ran during timing.

Common-workload ratios use equal-weight geometric means of per-case mean times.
Confidence intervals use 10,000 paired-bootstrap samples from the six rounds.
The random seed was 20261028.

| Workloads | Copy / borrow time (95% CI) | Copy / C time (95% CI) |
|---|---:|---:|
| General (11 cases) | 1.115 (1.067–1.163) | 0.830 (0.794–0.864) |
| Mail (7 cases) | 1.070 (1.027–1.113) | 0.853 (0.830–0.876) |

Copying increased general-workload time by about **11.5%** and mail time by
**7.0%** versus borrowing. It still used **17.0%** less time than C for general
cases and **14.7%** less for mail cases. All 38
cases had a lower mean time than C. Each case had an upper paired-bootstrap 95%
confidence bound below 1.0 for the Copy/C time ratio. The table shows selected
arithmetic means:

| Workload | Borrowing Rust µs/op | Copying Rust µs/op | C µs/op |
|---|---:|---:|---:|
| Plain text, two passes | 0.190 | 0.254 | 0.378 |
| No links (13,312 bytes) | 0.904 | 1.107 | 5.318 |
| Large mixed input | 632.870 | 587.517 | 837.919 |
| Reverse callbacks (64 links) | 16.527 | 16.394 | 18.072 |

Copying added the most relative time to the small no-link case. The large-input
speedup is an observation. It does not mean that copying is always faster.
Callback inputs already used owned snapshots.

Ruby allocations per operation matched in both Rust builds. The copying build
also stores source and attribute bytes in native buffers. This experiment did
not measure peak native memory. Local samples, library hashes, summaries, and
confidence intervals are in the ignored `benchmark/results/copying/` directory.

The copying build passed 73 Ruby tests, 11 GC-stress tests, and 11 Rust tests.
It also passed formatting, Clippy, and all 1,075 C differential observations.
We kept copying because it limits unsafe access to Ruby string storage. The
copying build also remained faster than C in these measurements.
