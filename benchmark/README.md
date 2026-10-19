# Benchmarks

Optional local measurements, not a CI job or release gate. The historical
[C-vs-Rust comparison](c-vs-rust.md) documents the rewrite's performance; it is
not a baseline for future runs.

From the repository root:

```sh
bundle exec rake compile
ruby benchmark/run.rb
# Longer samples, if needed:
SAMPLES=20 SAMPLE_SECONDS=0.2 WARMUP_SECONDS=0.1 ruby benchmark/run.rb
```

The runner measures the current extension in independent Ruby processes with
YJIT disabled and GC enabled. Each workload is warmed up and its expected output
checked before timing. JSON parsing and input preparation are outside timing.
It reports arithmetic mean time/op, throughput, and Ruby object allocations.
`results/raw.json` and `results/summary.json` are overwritten on each run and
gitignored. No previous results or historical C build are required. Set
`RINKU_LIB` to override the default `lib/` load path.

## Workloads

`workloads.rb` loads the fixtures in `fixtures/`, also used by the Ruby correctness
tests and optional C differential checks. Their expected outputs were verified
against C before the rewrite:

* `benchmark.json`: 11 cases timing the entire two-pass operation,
  `Rinku.auto_link(Rinku.auto_link(input, :urls, 'rel="noreferrer"'), :email_addresses)`.
* `mail_benchmark.json`: 7 extracted HTML text nodes, using the declared arguments
  `Rinku.auto_link(input, :urls, 'target="_blank"', nil)`. Extraction is not timed;
  email addresses remain unlinked.
* Supplemental cases cover no links, large inputs, reverse callbacks,
  mutable/frozen identity callbacks at 1,000–64,000 links, and repeated invalid
  email candidates at 1,000–16,000 repetitions.

## Native memory

Ruby allocation counts exclude native buffers. Profile separately from timings,
for example on Linux:

```sh
mkdir -p benchmark/results
heaptrack -o benchmark/results/memory ruby benchmark/memory.rb
```

On macOS use Instruments Allocations with native stack recording. Attribute
native allocations to extension call stacks, separately from Ruby VM allocations
and process RSS. Avoid concurrent builds, tests, and profilers when timing;
workstation measurements are exploratory, not controlled release qualification.
