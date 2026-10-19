# Review fixes: compatibility and scaling

Exploratory workstation measurements, 2026-10-19, Ruby 3.3.12, arm64 macOS,
YJIT disabled, release extension. C baseline:
`67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863`.

## Full benchmark

```sh
SAMPLES=6 SAMPLE_SECONDS=0.1 WARMUP_SECONDS=0.05 \
  ruby tools/benchmark.rb "$C_LOAD_PATH" tmp/review-benchmark --report-only
```

C and Rust ran sequentially in separate processes, alternating order each
round, with no concurrent builds or validation jobs. See [raw.json](raw.json)
for all samples, source/library/fixture hashes and environment metadata, and
[summary.json](summary.json) for every result and paired bootstrap interval.

All 32 local case intervals have an upper Rust/C bound below 1.0. The existing
64-link reverse-callback workload is now **0.972 (95% CI 0.953–0.993)**, compared
with 1.094 (1.074–1.116) in the earlier report. These are exploratory results;
controlled release qualification and native-memory profiling remain separate.

### Identity callbacks

Mean milliseconds per call on `('http://a.com ' * links)`:

| Links | Mutable C | Mutable Rust | Frozen C | Frozen Rust |
|---:|---:|---:|---:|---:|
| 1,000 | 0.173 | 0.160 | 0.174 | 0.156 |
| 4,000 | 0.662 | 0.619 | 0.656 | 0.617 |
| 16,000 | 2.629 | 2.524 | 2.656 | 2.414 |
| 64,000 | 10.776 | 9.864 | 10.987 | 9.697 |

Both source types now scale approximately linearly. These versioned workloads
use explicit expected output and run in the Ruby tests and C differential suite.

### Repeated invalid email candidates

Mean milliseconds per call on `'a@' * repetitions`, without a callback:

| Repetitions | C | Rust |
|---:|---:|---:|
| 1,000 | 0.788 | 0.021 |
| 4,000 | 12.086 | 0.085 |
| 16,000 | 190.806 | 0.341 |

## Isolated named-action scanner comparison

Before changing the binding, compared the pre-change release extension with a
release build containing the named-action/explicit-mode scanner, parser names
and limits, API documentation, and second-`@` rejection. None of the measured
inputs contained repeated `@` candidates, isolating the representation change
from that optimization and from callback snapshot changes.

Eight independent process pairs, alternating order; each case warmed up with
100 operations, then ran a calibrated batch targeting 100 ms. Cases were the
18 supplied fixtures plus `('plain www.a.com http://b.com a@b.com ' * 128)` in
each mode without callbacks. [scanner-raw.json](scanner-raw.json) retains all
per-process seconds/op (`before` and `named`). Ratios of arithmetic means:

* Supplied cases: 0.983–1.025.
* Mixed input: all 1.001, URLs 0.986, emails 0.983.

The representation change showed no material performance shift in this probe.

## Validation

* Rust core: 8 tests passed; formatting, Clippy and Rust 1.85 workspace check passed.
* Ruby: 67 tests / 248 assertions; optimized GC stress: 11 tests / 48 assertions.
  Includes embedded/shared source edits, replacement, shrinking/growing source,
  restoration to original length, freezing, recursion and compaction.
* C differential: 1,061 identical observations.
* Pinned Miri: all 8 core tests passed.
* Pinned nightly fuzzing: 218,238 executions in 31 seconds; identity callback
  equivalence and ordered/in-bounds ranges checked across modes and flags.
* A fresh Git-tree export containing the changed tracked files and both new
  fixtures, without local binaries, root JSONs or other untracked files, rebuilt
  and passed the Ruby/stress tests and all 1,061 C differential observations.
  Its source gem also installed and loaded successfully outside the checkout,
  including the ActionView integration smoke test.
