# Rust rewrite: validation and release qualification

## Architecture and safety boundary

`crates/rinku-core` contains the production scanner, HTML skipping, and renderer.
It has `#![forbid(unsafe_code)]`, no Ruby objects, and no UTF-8
requirement. It recognizes exactly the original complete whitespace byte
sequences; truncated sequences are ordinary bytes, never out-of-bounds reads.
Historical final-byte domain classification, `strchr`'s NUL behavior, Ruby's
`rb_ispunct == !rb_isalnum`, case sensitivity, and the punctuation iteration
limit are intentional compatibility rules, not opportunities for normalization.
Its only dependency is the safe byte-search API of pinned `memchr` 2.8.3;
bounded search chunks prevent repeated lookahead from becoming quadratic.

`ext/rinku` pins Magnus 0.9.3 and rb-sys 0.9.130. Rust 1.85 is the minimum
supported compiler (also checked in CI). The binding validates the C argument
types and validation order before entering the parser. Narrow protected rb-sys
calls preserve Ruby's exact Check_Type, FIX2INT, and StringValueCStr errors.
All execution remains under the GVL. Magnus's method/init wrappers catch Rust
panics. Callback exceptions, `break`, `return`, and `throw` pass through protected
calls and `Result`, dropping native allocations before reentering Ruby.

Ruby handles remain in VM/native-stack roots. Original skip-tag string objects
are retained in a **Ruby array**, never a `Vec<Value>`, so callback array edits
cannot make them unreachable. Cached IDs are permanent interned Ruby IDs using
Magnus `LazyId`, not unmarked heap values. There is no global mutable parser
state. Recursive calls have independent scanners, buffers, and snapshots.

The no-block fast path may borrow Ruby buffers only in a strictly Ruby-free
core call. Rust's system allocator cannot trigger Ruby GC. Callback processing
uses owned snapshots; callback results are copied into the owned renderer
before any subsequent Ruby call. Every unsafe binding operation documents its
rooting, GVL, protection, and lifetime assumptions inline.

## Callback mutation contract

These cases were characterized in a separate C process before deleting C:

* Same-length source edits affect future scanning/copying. After each callback,
  refresh the snapshot if the source still has the original byte length. Already
  emitted output and the current href are not rewritten.
* Attribute string edits affect later links. Preserve the original leading
  whitespace offset; stop at the current first NUL. Attribute resizing is safe
  in Rust, with that offset clamped to the new length.
* Skip-tag **string** edits affect later tag checks. Retain the original string
  objects and refresh their bytes after callbacks, stopping at NUL when parsing.
* Skip-tag **array** replacement, insertion, or removal does not replace the
  originally loaded tag objects. The binding roots them even if removed.
* Length-changing source mutation can make C access freed buffers or read beyond
  the new string. Such behavior is not a baseline contract. Rust keeps the last
  same-length snapshot when the length changes. Same-length edits use the current
  bytes regardless of whether Ruby reallocated the buffer. Callback arguments use
  the initial encoding; final output uses the source's current encoding, as in C.

These rules preserve defined in-place mutation while making previously unsafe
resizing and collection deterministic. Mutation never leaves a Ruby buffer
pointer alive across callback execution, allocation, or compaction.

## Reproduce the C baseline

The baseline is **67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863**.
`docs/baseline.json` records the characterization environment and invocation.
The repository contains no C fallback or duplicate reference implementation.
Git history is sufficient; `tools/build_baseline.rb` exports only historical
`ext/` and `lib/` into a **new external directory**, and builds with the exact
Ruby executable running the script. It records build metadata alongside it.
Fetch full history first if using a shallow clone.

```sh
ruby tools/build_baseline.rb /absolute/new/path/rinku-c
bundle exec rake compile
ruby tools/differential.rb /absolute/new/path/rinku-c/lib:/absolute/new/path/rinku-c/ext/rinku
```

Alternatively pass load paths for a separately installed C gem. The harness
launches separate Ruby processes against identical fixtures. It compares output
bytes, class, encoding, identity, strict errors/messages, validation order,
callback arguments/order, mutations, module inclusion, all byte-prefix classes,
and a fixed-seed randomized corpus. Inputs that could make C's Unicode lookahead
read out of bounds are tested only against the safe core, not used as C oracles.

## Datasets and measurements

The JSON files remain unchanged. Every supplied expected output was verified
against C before adoption:

* `benchmark.json` (11 cases):
  `Rinku.auto_link(Rinku.auto_link(input, :urls, 'rel="noreferrer"'), :email_addresses)`.
  The **entire two-pass operation** is timed. This resolves URL-only attributes
  without changing the public API or email expectations.
* `mail_benchmark.json` (7 cases): consume its declared arguments verbatim:
  `Rinku.auto_link(input, :urls, 'target="_blank"', nil)`.
  Inputs are already extracted, unlinked HTML text nodes; no HTML extraction is
  included in measurements. Email addresses stay unlinked.
* Supplemental cases: large input, no links, and repeated callback rendering.

```sh
SAMPLES=20 SAMPLE_SECONDS=0.2 WARMUP_SECONDS=0.1 \
  ruby tools/benchmark.rb /absolute/new/path/rinku-c/lib:/absolute/new/path/rinku-c/ext/rinku tmp/measurements
```

The harness first runs differential correctness checks outside timing. It warms
up every case, keeps GC enabled with identical settings, disables YJIT in both
processes, alternates C/Rust process order, and samples independent processes.
JSON parsing and preparation are outside timed sections. `raw.json` records
every elapsed time, iteration count, throughput, and Ruby allocation count,
plus Ruby/Rust/compiler versions, source and native-library hashes, revisions,
build settings, platform, fixture hashes, and invocation. `summary.json` includes
per-case throughput, bootstrap 95% confidence intervals for mean time/op, and
paired ratios of arithmetic mean times. Dataset and combined aggregates give every case equal
weight (geometric means of ratios); they do not replace the per-case gate.

The **zero-tolerance gate** requires the upper confidence bound of Rust/C time
to be **at most 1.0 on every workload**. Crossing 1.0 is inconclusive, not a pass.
The command exits unsuccessfully unless all pass. `--report-only` saves
exploratory results without implying release qualification. Shared CI is for
reproduction/artifacts only; use dedicated controlled hardware to qualify a
release. Do not run compilers, stress tests, or profilers concurrently with
timing. More samples do not repair uncontrolled systematic noise.

Ruby object counts do **not** measure Rust allocations. Profile native memory
separately, e.g. on Linux with heaptrack, using the same Ruby and built libraries:

```sh
RINKU_LIB=/absolute/new/path/rinku-c/lib:/absolute/new/path/rinku-c/ext/rinku \
  heaptrack -o c-memory ruby tools/memory_worker.rb
RINKU_LIB="$PWD/lib" heaptrack -o rust-memory ruby tools/memory_worker.rb
heaptrack_print c-memory.*.gz > c-memory.txt
heaptrack_print rust-memory.*.gz > rust-memory.txt
```

Keep allocation counts, peak native bytes, and retained allocations attributed
to the extension's call stacks separate from Ruby VM allocations and process
RSS. On macOS use Instruments Allocations with native stack recording instead.
The benchmark workflow collects native profiles separately from timing files.

## Safety validation and packaging

```sh
bundle exec rake test gc_stress
cargo test -p rinku-core --locked
rustup component add --toolchain nightly-2026-10-01 miri
cargo +nightly-2026-10-01 miri test -p rinku-core --locked
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly-2026-10-01 fuzz run autolink -- -max_total_time=60 -max_len=16384
ruby tools/package_smoke.rb
```

Miri validates actual production core paths, not Ruby/libruby. Optimized Ruby
stress tests cover temporary/embedded/heap strings, invalid encodings, compaction,
heavy allocations, recursive calls, strict callback results, exceptions,
nonlocal exits, mutation, and no-link identity. They restore `GC.stress` in
`ensure` and run separately from timing. Fuzzing is bounded; move minimized
failures into `crates/rinku-core/tests/core.rs` as permanent regressions.

The source gem explicitly includes both workspace manifests, member manifests,
Rust sources, `Cargo.lock`, license, and public Ruby files. `package_smoke.rb`
builds and installs it into an isolated temporary GEM_HOME and verifies native
loading and current ActionView integration from outside the checkout.

## Release checklist

The version is `2.0.0.pre.1`; no gem is published by these local tools.

Local validation: Ruby tests, 1,049 differential observations, optimized GC
stress/compaction, Rust tests, Clippy/formatting, Rust 1.85 compilation, pinned
Miri, bounded fuzzing, and source-gem loading with ActionView on Ruby 3.3/3.4/4.0.
See [the exploratory performance report](benchmarks/2026-10-19-final/README.md):
all supplied dataset cases pass locally, but the callback-heavy supplemental
case is about 9.4% slower. **The no-slowdown release requirement is not met.**
Native allocation profiling and controlled hardware qualification remain open.

- Pass Ruby 3.3, 3.4, and 4.0 compatibility, optimized stress, and package CI.
- Pass pinned Miri, formatting, Clippy, MSRV, bounded properties, and fuzzing.
- Recheck differential observations using the recorded C revision.
- Collect controlled performance samples and separate native memory profiles.
- Require the zero-slowdown gate on **all** required and supplemental workloads.
- Review evidence before manually publishing a prerelease; qualify that
  prerelease before deciding on final 2.0. Do not publish merely because an
  aggregate improved or noisy CI failed to detect a slowdown.
