# Benchmarks

These scripts measure performance on your local machine. CI does not run them,
and they are not release gates. The historical
[C-versus-Rust report](c-vs-rust.md) is not a baseline for future runs.

From the repository root:

```sh
bundle exec rake compile
ruby benchmark/run.rb
```

To collect longer samples, run:

```sh
SAMPLES=20 SAMPLE_SECONDS=0.2 WARMUP_SECONDS=0.1 ruby benchmark/run.rb
```

The runner starts a separate Ruby worker process for each sample round. Each
worker measures all workloads with YJIT disabled and the garbage collector
enabled. It warms up each workload and checks its expected output before
timing. It does not time JSON parsing or input setup. The runner reports mean
time per operation, throughput, and Ruby object allocations. Each run
overwrites `results/raw.json` and
`results/summary.json`. Git ignores these files. You do not need old results or
a historical C build. Set `RINKU_LIB` to use a different load path instead of
the default `lib/` path.

## Workloads

`workloads.rb` loads fixtures from `fixtures/`. Ruby correctness tests and
optional C compatibility checks use the same fixtures. The expected outputs
were checked against C before the rewrite.

* `benchmark.json` has 11 cases. Each case times the full two-pass operation:
  `Rinku.auto_link(Rinku.auto_link(input, :urls, 'rel="noreferrer"'), :email_addresses)`.
* `mail_benchmark.json` has 7 extracted HTML text nodes. Each case calls
  `Rinku.auto_link(input, :urls, 'target="_blank"', nil)`. The runner does not
  time HTML extraction. It does not link email addresses in these cases.
* Supplemental cases test no links, large input, reverse callbacks, mutable and
  frozen identity callbacks with 1,000 to 64,000 links, and repeated invalid
  email candidates with 1,000 to 16,000 repetitions.

## Native memory

Ruby allocation counts do not include native buffers. Profile native memory
separately from timing. On Linux, for example, run:

```sh
mkdir -p benchmark/results
heaptrack -o benchmark/results/memory ruby benchmark/memory.rb
```

On macOS, use Instruments Allocations with native stack recording. Find native
allocations in extension call stacks. Do not confuse them with Ruby VM
allocations or process RSS. Do not run builds, tests, or profilers at the same
time as a timing run. Workstation results are exploratory. They are not
controlled release qualification evidence.
