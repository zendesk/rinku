# Benchmarks

These scripts measure local performance. CI does not run them, and benchmark
results are not release gates. The historical
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

The runner starts one Ruby worker process for each sample round. Each worker
runs every workload with YJIT disabled and the garbage collector enabled. It
warms up each workload and checks its expected output before timing. Timings do
not include JSON parsing or input setup. The runner reports mean time per
operation, throughput, and Ruby object allocations. Each run overwrites
`results/raw.json` and `results/summary.json`. Git ignores these files. You do
not need earlier results or a historical C build. Set `RINKU_LIB` to use a
different load path. The default path is `lib/`.

## Workloads

`workloads.rb` loads fixtures from `fixtures/`. Ruby correctness tests and
optional C compatibility checks use the same fixtures. We checked the expected
outputs against C before the rewrite.

* `benchmark.json` has 11 cases. Each case times the full two-pass operation:
  `Rinku.auto_link(Rinku.auto_link(input, :urls, 'rel="noreferrer"'), :email_addresses)`.
* `mail_benchmark.json` has 7 extracted HTML text nodes. Each case calls
  `Rinku.auto_link(input, :urls, 'target="_blank"', nil)`. The runner does not
  time HTML extraction. It does not link email addresses in these cases.
* Supplemental cases test no links and large inputs. They also measure reverse
  callbacks and mutable and frozen identity callbacks with 1,000 to 64,000
  links. Other cases test repeated invalid email candidates, adjacent URL and
  HTML fragments, and rejected `www.` candidates. These cases use 1,000 to
  16,000 repetitions.

## Native memory

Ruby allocation counts exclude native buffers. Profile native memory in a
separate run. On Linux, for example, run:

```sh
mkdir -p benchmark/results
heaptrack -o benchmark/results/memory ruby benchmark/memory.rb
```

On macOS, use the Allocations instrument in Instruments. Enable native stack
recording. Check extension call stacks for native allocations. Do not confuse
native allocations with Ruby VM allocations or process RSS. Do not run builds,
tests, or profilers during a timing run. Treat workstation results as
exploratory. They are not controlled release qualification evidence.
