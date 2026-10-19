# Why the Rust rewrite: historical performance

The rewrite improves common autolinking workloads while moving the production
scanner and renderer into a safe Rust core (`#![forbid(unsafe_code)]`). In the
2026-10-19 comparison, general workloads used **22% less time** and mail text-node
workloads **18% less time** than C (equal-weight geometric means of Rust/C times:
0.779 and 0.823). Repeated invalid email candidates improved from quadratic to
approximately linear scaling.

These are historical observations, not a maintained baseline or release gate.
Raw samples and intermediate reports are not retained in the working tree; Git
history contains the original reports under `docs/benchmarks/`.

## Measurement context

Ruby 3.3.12 on arm64 macOS, release extension, YJIT disabled, GC enabled.
Historical C revision: `67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863`.
Six independent C/Rust process pairs ran sequentially in alternating order,
with 100 ms target samples and 50 ms warmups per case, without concurrent builds
or validation jobs. Output correctness was checked before timing; 1,061
differential observations matched. Times below are arithmetic means.

All 32 measured cases had an upper paired-bootstrap 95% confidence bound for
Rust/C time below 1.0. This is exploratory workstation evidence, not a claim
about every platform or controlled release qualification. Native allocation
profiles were not collected; Ruby object counts do not measure native memory.

## Supplied workloads

General cases time the full URL-then-email two-pass operation. Mail cases time
URL-only linking of already extracted text nodes; HTML extraction is excluded.
See [workload definitions](README.md#workloads).

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

The reverse-callback Rust/C ratio was **0.972 (95% CI 0.953–0.993)** after the
callback fixes, superseding the earlier exploratory regression of 1.094
(1.074–1.116).

Identity callbacks on `'http://a.com ' * links`, milliseconds per call:

| Links | Mutable C | Mutable Rust | Frozen C | Frozen Rust |
|---:|---:|---:|---:|---:|
| 1,000 | 0.173 | 0.160 | 0.174 | 0.156 |
| 4,000 | 0.662 | 0.619 | 0.656 | 0.617 |
| 16,000 | 2.629 | 2.524 | 2.656 | 2.414 |
| 64,000 | 10.776 | 9.864 | 10.987 | 9.697 |

Both source types scale approximately linearly. Repeated invalid email
candidates (`'a@' * repetitions`, no callback), milliseconds per call:

| Repetitions | C | Rust |
|---:|---:|---:|
| 1,000 | 0.788 | 0.021 |
| 4,000 | 12.086 | 0.085 |
| 16,000 | 190.806 | 0.341 |

At 16,000 repetitions Rust was about **560× faster**, avoiding C's quadratic
candidate rejection. This adversarial case is not representative of typical
mail throughput and is not folded into the common-workload headline above.
