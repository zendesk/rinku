# Rust rewrite: validation and release qualification

## Architecture and safety boundary

`crates/rinku-core` contains the production scanner, HTML tag skipping, and
renderer. It forbids unsafe Rust. It does not use Ruby objects or require
UTF-8 input. The scanner works on bytes. It accepts the same complete
whitespace sequences as the C version. It treats truncated sequences as
ordinary bytes and does not read past the input.

The public `Scanner` and `Renderer` APIs use byte offsets. Keep the input length
unchanged while scanning. You can change unread bytes between calls, but the
scanner does not revisit earlier ranges. For each link, call `start_link` and
then `end_link` once. Pass `start_link` the snapshot that the scanner used for
that link. Call `finish` once with the latest snapshot. A shorter final
snapshot truncates the output suffix.

The core preserves several historical rules. These include final-byte domain
classification, `strchr` matching a terminating NUL, Ruby's
`rb_ispunct == !rb_isalnum` behavior, case-sensitive `www.` and HTML tag names,
and the punctuation iteration limit. Do not normalize these rules. The core has
one dependency: `memchr` 2.8.3. Its safe byte-search API uses 256-byte chunks.
This prevents repeated lookahead from scanning the full unread suffix each time.

`ext/rinku` pins Magnus 0.9.3 and rb-sys 0.9.130. Rust 1.85 is the minimum
supported compiler. CI checks this version. The binding checks argument types
and validation order before it calls the parser. Small protected rb-sys calls
preserve Ruby's exact `Check_Type`, `FIX2INT`, and `StringValueCStr` errors.

All binding calls run under the Global VM Lock (GVL). Magnus's method and init
wrappers catch Rust panics. Protected calls return callback exceptions,
`break`, `return`, and `throw` through `Result`. This lets Rust drop native
allocations before control returns to Ruby.

Keep Ruby handles in VM or native-stack roots. Keep the original skip-tag
strings in a **Ruby array**, not in `Vec<Value>`. A callback can change its
array, but the Ruby array keeps the original strings marked. `LazyId` stores
permanent interned Ruby IDs, not unmarked heap values. The parser has no global
mutable state. Each recursive call has its own scanner, buffers, and snapshots.

**Never borrow Ruby string storage in the binding or core.** Source text,
attributes, skip tags, and callback results all pass through `bytes()` before
their contents are used. It uses Magnus's safe `RString::to_bytes()` conversion
(the `bytes` feature), then transfers the owned allocation into `Vec<u8>`.
This preserves arbitrary bytes without transcoding or calling Ruby overrides.
Only Rust-owned byte slices reach the scanner and renderer. Magnus handles the
short internal borrow needed to copy; the binding creates no references into
Ruby string storage. Each remaining unsafe binding operation states its rooting,
GVL, protection, and lifetime assumptions.

Frozen input uses an owned snapshot and needs no refresh checks. Mutable input
also uses a frozen Ruby copy-on-write alias rooted on the native stack. Ruby
must detach shared storage before it can edit the source. The binding can then
compare buffer addresses in constant time instead of copying unchanged text
after every callback. `shares_storage()` returns only a boolean: it does not
dereference the pointers or turn them into slices. If the source keeps its
original length and the pointers differ, the binding takes an owned copy,
refreshes unread bytes, and creates a new alias. Embedded strings require a
bounded copy. Pointers exist only in the identity check and never survive a
callback or allocation.
Pointer equality alone would miss edits without the frozen alias.

### Enforcing owned string access

`clippy.toml` disallows Magnus's string-borrowing methods, including byte and
text slices, lossy text views, and borrowing iterators. It also disallows the
raw rb-sys string-pointer accessors. The binding denies
`clippy::disallowed_methods`; CI runs this with the usual command:

```sh
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Clippy resolves API identities, so import aliases and fully qualified calls
are checked too. There are two expression-local exceptions: the pointer-only
identity check in `shares_storage()` and `StringValueCStr` tag validation,
which discards the returned pointer. Neither exposes a borrowed byte view.
Keep these exceptions narrow. This is an API guardrail, not a proof against
arbitrary new unsafe code; review the list when changing binding dependencies.

## Callback mutation contract

We recorded these cases in a separate C process before we removed the C code:

* Same-length source edits affect later scanning and copying. After each
  callback, refresh the snapshot if the source still has its original byte
  length. Do not rewrite output that is already emitted or the current `href`.
* Attribute edits affect later links. Keep the original leading-whitespace
  offset and clamp it to the current string length. Stop at the first NUL.
* Edits to a skip-tag **string** affect later tag checks. Keep the original
  string objects and refresh their bytes after each callback. Treat a NUL as
  the end of a tag when parsing.
* Replacing, adding, or removing items in the caller's skip-tag **array** does
  not replace the strings that the call loaded at the start. The binding keeps
  those strings rooted, even if the caller removes them.
* A source length change can make C read freed memory or read past the new
  string. This is not a compatibility contract. Rust keeps the last snapshot
  until the source returns to its original length. For same-length edits, Rust
  uses current bytes even if Ruby moved the buffer.
* Callback arguments use the source's initial encoding. Final output uses the
  source's current encoding, as in C.

These rules keep defined in-place changes and make unsafe resizing and
collection deterministic. The binding does not keep a Ruby buffer pointer
across a callback, allocation, or compaction.

## C compatibility comparison

The historical C revision is **67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863**.
The repository has no C fallback or duplicate reference implementation. Use
Git history to build the old version. `tools/build_baseline.rb` exports only
the historical `ext/` and `lib/` directories to a **new external directory**.
It builds the extension with the Ruby executable that runs the script. It also
records build metadata. Fetch the full Git history before you use this script
in a shallow clone.

```sh
ruby tools/build_baseline.rb /absolute/new/path/rinku-c
bundle exec rake compile
ruby tools/differential.rb /absolute/new/path/rinku-c/lib:/absolute/new/path/rinku-c/ext/rinku
```

You can also pass load paths for an installed C gem. The harness starts a
separate Ruby process for each implementation and uses the same fixtures. It
compares output bytes, class, encoding, identity, errors and messages,
validation order, callback arguments and order, mutations, module inclusion,
all byte-prefix classes, and a fixed-seed randomized corpus. It tests inputs
that could make C's Unicode lookahead read past the input only against the
safe core. It does not use C as an oracle for those inputs.

## Performance

Benchmark scripts, fixtures, and documentation are in
[`benchmark/`](../benchmark/README.md). Benchmarks are optional local tools.
They do not run in CI and are not release gates. The runner overwrites results
on each run, and Git ignores those files. You do not need old results or a C
build to run the benchmarks. Correctness tests and C compatibility checks use
the same workload fixtures. Their expected outputs have not changed.

The [historical C-versus-Rust report](../benchmark/c-vs-rust.md) lists the
measured performance changes and limits. It does not keep raw results as a
permanent baseline.

## Safety validation and packaging

```sh
bundle exec rake test gc_stress
cargo test -p rinku-core --locked
rustup component add --toolchain nightly-2026-10-01 miri
cargo +nightly-2026-10-01 miri test -p rinku-core --locked
cargo install cargo-fuzz --version 0.13.2 --locked
cargo +nightly-2026-10-01 fuzz run autolink -- -dict=fuzz/autolink.dict -max_total_time=60 -max_len=16384
ruby tools/package_smoke.rb
```

Miri tests production core paths, not Ruby or `libruby`. Optimized Ruby stress
tests cover temporary, embedded, and heap strings; invalid encodings; garbage
collector compaction; heavy allocation; recursive calls; strict callback
results; exceptions; nonlocal exits; mutation; and no-link identity. The tests
restore `GC.stress` in `ensure`. Run them separately from timing tests. Fuzzing
uses the committed `fuzz/autolink.dict` dictionary and has a time limit. Run the
commands above from the repository root. Add each minimized failure to
`crates/rinku-core/tests/core.rs` as a regression test.

The source gem includes the root workspace manifest, the crate manifests, Rust
sources, `Cargo.lock`, the license, and public Ruby files. `package_smoke.rb`
builds and installs the gem in a temporary `GEM_HOME`. It then checks native
loading and the current ActionView integration from outside the repository.

## Release checklist

The current version is `2.0.0.pre.1`. Local tools do not publish the gem. The
GitHub Actions publish workflow runs manually. It requires an HTTPS URL for
reviewed, controlled performance and native-memory qualification evidence.

Current local validation includes Ruby tests, 1,075 differential observations,
optimized garbage-collector stress and compaction, Rust tests, Clippy,
formatting, Rust 1.85 compilation, pinned Miri, bounded fuzzing, and source-gem
loading with ActionView on Ruby 3.3, 3.4, and 4.0. The
[C-versus-Rust report](../benchmark/c-vs-rust.md) summarizes historical
performance. That comparison did not include native-memory profiling or
controlled hardware measurements.

- Pass Ruby 3.3, 3.4, and 4.0 compatibility tests, optimized stress tests, and
  package tests.
- Pass pinned Miri, formatting, Clippy, MSRV, bounded property tests, and
  fuzzing.
- Repeat the differential checks with the recorded C revision.
- Review the evidence before you run the publish workflow. Qualify the
  pre-release before you decide to publish final 2.0. Historical benchmark
  gains alone do not qualify a release.
