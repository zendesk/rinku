# Rust rewrite: validation and release qualification

## Architecture and safety boundary

`crates/rinku-core` contains the production scanner, HTML tag skipper, and
renderer. The crate forbids unsafe Rust and does not use Ruby objects. The
scanner reads bytes and accepts invalid UTF-8. It accepts the same complete
whitespace sequences as the C version. It treats truncated sequences as
ordinary bytes and does not read past the input.

The public `Scanner` and `Renderer` APIs use byte offsets. Keep the input length
unchanged during a scan. You can change unread bytes between calls. The scanner
offset only moves forward. For each link, call
`start_link` once, then call `end_link` once. Pass `start_link` the input
snapshot that the scanner used for that link. Call `finish` once with the latest
input snapshot. If the final snapshot is shorter, `finish` truncates the output
suffix.

The core preserves these C rules:

* Domain checks do not classify the final byte.
* `strchr` matches the terminating NUL.
* Ruby's `rb_ispunct` behavior is `!rb_isalnum`.
* `www.` and HTML tag names are case-sensitive.
* Punctuation trimming has a fixed iteration limit.

Do not normalize these rules. The core has one dependency: `memchr` 2.8.3.
`memchr` provides a safe byte-search API. The scanner calls it on 256-byte
chunks. This prevents each lookahead from scanning the full unread suffix.

`ext/rinku` pins Magnus 0.9.3 and rb-sys 0.9.130. Rust 1.85 is the minimum
supported compiler. CI checks this version. The binding checks argument types
and validation order before parsing. Small protected rb-sys calls preserve
Ruby's exact `Check_Type`, `FIX2INT`, and `StringValueCStr` errors.

The binding runs every Ruby call under the Global VM Lock (GVL). Magnus catches
Rust panics in its method and init wrappers. Protected calls return callback
exceptions and nonlocal exits as `Result`. These exits include `break`,
`return`, and `throw`. Rust can then drop native allocations before control
returns to Ruby.

Keep Ruby handles in VM roots or on the native stack. The binding keeps the
original skip-tag strings in a **Ruby array**, not in `Vec<Value>`. It
duplicates the caller's array before scanning. A callback can change the
caller's array, but the rooted copy keeps the original strings visible to GC.
`LazyId` stores permanent, interned Ruby IDs. It does not store unmarked heap
values. The parser has no global mutable state. Each recursive call has its own
scanner, buffers, and snapshots.

**Never borrow Ruby string storage in the binding or core.** Source text,
attributes, skip tags, and callback results pass through `bytes()` before use.
This function uses Magnus's safe `RString::to_bytes()` conversion (the `bytes`
feature), then moves the owned allocation into `Vec<u8>`. This preserves
arbitrary bytes without transcoding or calling Ruby overrides. Only Rust-owned
byte slices reach the scanner and renderer. Magnus uses a short internal borrow
to copy the bytes. The binding creates no references into Ruby string storage.
Each unsafe binding operation states its rooting, GVL, protection, and lifetime
assumptions.

Frozen input needs only an owned snapshot. It needs no refresh check. For
mutable input, the binding also creates a frozen Ruby alias and roots it on the
native stack. Ruby must detach shared storage before it can edit the source. For
embedded strings, Ruby copies the buffer instead of sharing it. The binding
compares buffer addresses to detect a write. This check takes constant time and
does not copy unchanged text after each callback. `shares_storage()` returns a
boolean. It does not read through the pointers or create slices. If the source
has its original length and the pointers differ, the binding takes an owned
copy, refreshes unread bytes, and creates a new alias. An embedded string needs
a bounded copy. Pointers exist only during the identity check. They do not
survive a callback or allocation. Pointer equality alone would miss edits
without the frozen alias.

### Enforcing owned string access

`clippy.toml` blocks Magnus methods that borrow string data. These methods
include byte and text slices, lossy text views, and borrowing iterators. It also
blocks raw rb-sys string-pointer accessors. The binding denies
`clippy::disallowed_methods`. CI runs:

```sh
cargo clippy --workspace --all-targets --locked -- -D warnings
```

Clippy resolves API identities. It also checks import aliases and fully
qualified calls. Two local exceptions remain:

* `shares_storage()` uses pointers only to compare addresses.
* `StringValueCStr` validates tag strings. The code discards its returned pointer.

Neither exception exposes a borrowed byte view. Keep both exceptions narrow.
This guardrail does not prove that new unsafe code is safe. Review the block
list when you change binding dependencies.

## Callback mutation contract

Before we removed the C code, we used a separate C process to record these cases:

* Same-length source edits affect later scans and copies. After each callback,
  refresh the snapshot only if the source keeps its original byte length. Do not
  change emitted output or the current `href`.
* Attribute edits affect later links. Keep the original leading-whitespace offset
  and clamp it to the current attribute length. Stop reading at the first NUL.
* Edits to a skip-tag **string** affect later tag checks. Keep the original
  string objects and refresh their bytes after each callback. Treat a NUL as the
  end of a tag.
* Replacing, adding, or removing items in the caller's skip-tag **array** does
  not replace the strings that the call loaded at the start. The binding keeps
  those strings rooted, even if the caller removes them.
* A source-length change can make C read freed memory or read past the new
  string. This is not a compatibility rule. Rust keeps the last snapshot until
  the source returns to its original length. For same-length edits, Rust uses
  current bytes even if Ruby moves the buffer.
* Callback arguments use the source's initial encoding. Final output uses the
  source's current encoding, as in C.

These rules define in-place changes and make resize and garbage collection
behavior deterministic. The binding does not keep a Ruby buffer pointer across
a callback, allocation, or GC compaction.

## C compatibility comparison

The historical C revision is **67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863**.
The repository has no C fallback or duplicate reference implementation. Use
Git history to build the old version. `tools/build_baseline.rb` exports only
the old `ext/` and `lib/` directories to a **new external directory**. The
script builds the extension with the Ruby executable that runs it. It also
records build metadata. Fetch the full Git history before you run this script
in a shallow clone.

```sh
ruby tools/build_baseline.rb /absolute/new/path/rinku-c
bundle exec rake compile
ruby tools/differential.rb /absolute/new/path/rinku-c/lib:/absolute/new/path/rinku-c/ext/rinku
```

You can also pass load paths for an installed C gem. The harness starts one Ruby
process for each implementation. It uses the same fixtures for both versions.
It compares output bytes, classes, encodings, object identity, errors, and
messages. It also checks validation order, callback arguments and order,
mutations, module inclusion, every byte-prefix class, and a fixed-seed random
corpus. The harness tests inputs that could make C's Unicode lookahead read
past the input against the safe core only. It does not use C as the expected
result for these inputs.

## Performance

See [`benchmark/`](../benchmark/README.md) for benchmark scripts, fixtures, and
documentation. These optional tools run locally. CI does not run them, and they
are not release gates. Each run overwrites result files. Git ignores those
files. You do not need old results or a C build to run the benchmarks.
Correctness tests and C compatibility checks use the same workload fixtures.
Their expected outputs have not changed.

The [historical C-versus-Rust report](../benchmark/c-vs-rust.md) lists measured
performance changes and limits. It does not keep raw results as a permanent
baseline.

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
tests cover:

* temporary, embedded, and heap strings;
* invalid encodings and garbage collector compaction;
* heavy allocation and recursive calls;
* strict callback results, exceptions, and nonlocal exits;
* mutation and identity when no link exists.

The tests restore `GC.stress` in `ensure`. Run them separately from timing
tests. Fuzzing uses the committed `fuzz/autolink.dict` dictionary and has a time
limit. Run the commands above from the repository root. Add each minimized
failure to `crates/rinku-core/tests/core.rs` as a regression test.

The source gem includes the root workspace manifest, crate manifests, Rust
source files, `Cargo.lock`, the license, and public Ruby files.
`package_smoke.rb` builds and installs the gem in a temporary `GEM_HOME`. It
then checks native loading and the current ActionView integration from outside
the repository.

## Release checklist

The current version is `2.0.0.pre.1`. Local tools do not publish the gem. You
must start the GitHub Actions publish workflow manually. The workflow requires
an HTTPS URL to reviewed qualification evidence. The evidence must cover
controlled performance and native-memory tests.

Current local validation includes:

* Ruby tests and 1,075 differential observations;
* optimized garbage-collector stress and compaction;
* Rust tests, Clippy, formatting, and Rust 1.85 compilation;
* pinned Miri and bounded fuzzing;
* source-gem loading with ActionView on Ruby 3.3, 3.4, and 4.0.

The [C-versus-Rust report](../benchmark/c-vs-rust.md) summarizes historical
performance. That comparison did not include native-memory profiling or
controlled hardware measurements.

- Pass compatibility, optimized stress, and package tests on Ruby 3.3, 3.4,
  and 4.0.
- Pass pinned Miri, formatting, Clippy, MSRV, bounded property tests, and
  fuzzing.
- Repeat the differential checks with the recorded C revision.
- Review the evidence before you start the publish workflow. Qualify the
  pre-release before you publish version 2.0. Historical benchmark gains alone
  do not qualify the release.
