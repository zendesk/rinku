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

Frozen sources retain their owned snapshot without refresh checks. Mutable
sources additionally keep a native-stack-rooted frozen Ruby copy-on-write alias.
While source and alias share storage, Ruby must detach before editing, so their
current buffer pointers can be compared in constant time without scanning the
unread suffix. Detached buffers are copied and the alias renewed; embedded
strings use bounded copies. Pointers are obtained afresh in a Ruby-free scope,
never retained across callbacks or allocation. Pointer equality without this
immutable alias would miss in-place edits and is not a valid write detector.

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

## C compatibility comparison

The historical C revision is **67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863**.
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

## Performance

All benchmark scripts, fixtures, and documentation live in [`benchmark/`](../benchmark/README.md).
Benchmarks are optional local tools, not CI jobs or release gates. Results are
gitignored and overwritten; no historical results or C build are needed to run
them. Correctness tests and C differential checks still share the workload
fixtures, whose expected outputs remain unchanged.

The [historical C-vs-Rust comparison](../benchmark/c-vs-rust.md) records the
rewrite's performance improvements and measurement limitations without keeping
raw results as a permanent baseline.

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

Local validation: Ruby tests, 1,061 differential observations, optimized GC
stress/compaction, Rust tests, Clippy/formatting, Rust 1.85 compilation, pinned
Miri, bounded fuzzing, and source-gem loading with ActionView on Ruby 3.3/3.4/4.0.
Historical performance observations are summarized in the
[C-vs-Rust comparison](../benchmark/c-vs-rust.md). Native allocation profiling
and controlled hardware measurements were not part of that comparison.

- Pass Ruby 3.3, 3.4, and 4.0 compatibility, optimized stress, and package CI.
- Pass pinned Miri, formatting, Clippy, MSRV, bounded properties, and fuzzing.
- Recheck differential observations using the recorded C revision.
- Review evidence before manually publishing a prerelease; qualify that
  prerelease before deciding on final 2.0. Historical benchmark improvements
  alone are not release qualification.
