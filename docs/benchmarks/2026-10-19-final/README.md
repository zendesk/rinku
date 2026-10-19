# Exploratory measurements — not release qualification

Ruby 3.3.12, arm64 macOS, release Rust 1.99.1, pinned C revision
`67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863`. Twelve independent alternating
process pairs, 80 ms target samples, 30 ms warmup per case; GC enabled, YJIT
disabled. Correctness matched across 1,048 observations before timing.

| Dataset | Equal-weight geometric mean Rust/C time |
|---|---:|
| General (11 cases; full two-pass operation) | 0.777 |
| Mail text nodes (7 cases) | 0.793 |
| Combined, including 3 supplemental cases | 0.736 |

All **18 supplied dataset cases** have an upper 95% confidence bound below 1.0
in this run. The no-link and large-input supplemental cases also pass.

**The release gate fails:** the callback-heavy supplemental case is **1.094×**
the C time, with a 95% interval of **1.074–1.116**. The aggregate improvement
does not excuse this regression. Further callback-boundary optimization and
controlled release measurements are required; no nonzero tolerance was adopted.

`raw.json` contains every sample and source/native-library hashes. `summary.json`
contains every case's time/op, throughput, Ruby object allocations, bootstrap
intervals, and gate result. Ratios use arithmetic mean times, bootstrapping
whole C/Rust sample pairs. Native allocation profiles must be collected
separately; they were not measured on this local machine. The dedicated benchmark
workflow records heaptrack profiles separately from timings.

This is a developer workstation, not controlled release hardware. These results
are evidence for optimization, not permission to publish a qualified 2.0 release.
