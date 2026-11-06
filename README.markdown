# Rinku

Rinku is a Ruby gem that turns URLs and email addresses in text into HTML links.
Zendesk owns and maintains this gem.

## Install

Install the pre-release from RubyGems:

```sh
gem install zendesk-rinku --pre
```

Version 2.0 requires **CRuby 3.3 or later**. A source build requires Rust and
Cargo **1.85 or later**. It also requires Ruby development headers, a C/C++
compiler, `make`, and libclang. The `rb-sys` binding generator uses libclang.
Install Rust with [rustup](https://rustup.rs/). On Debian or Ubuntu, install
`build-essential`, `ruby-dev`, and `libclang-dev`. On macOS, install the Xcode
command-line tools. If the build cannot find libclang, install LLVM. Set
`LIBCLANG_PATH` to the libclang location when necessary.

This branch is a pre-release candidate. Release qualification is not complete.
Read [the validation notes](docs/rust-rewrite.md).

Clone the source repository:

```sh
git clone https://github.com/zendesk/rinku.git
```

## Use Rinku

Require `rinku` and call `Rinku.auto_link`:

```ruby
require 'rinku'

Rinku.auto_link(text)
Rinku.auto_link(text) { |link_text| link_text }
```

Rinku scans plain text or escaped HTML. It does not escape input. Escape HTML
before you call `auto_link`. Rinku skips content inside `<a>`, `<pre>`, `<code>`,
`<kbd>`, and `<script>` tags by default. This prevents links inside existing
anchors and these other tags.

Rinku links URLs that use `http://`, `https://`, or `ftp://`. It also links email
addresses and uses `mailto:` in their `href` values. Rinku adds `http://` to
URLs that start with lowercase `www.`.

The arguments are:

* `text`: a Ruby `String` with plain text or escaped HTML.
* `mode`: a `Symbol`. Use `:all`, `:urls`, or `:email_addresses`. `nil` selects
  `:all`.
* `link_attr`: a `String` of HTML attributes to add to each link. Rinku does not
  sanitize this string. Use trusted attributes.
  In Rails, use `tag_options` to build this string.
* `skip_tags`: an `Array` of tag-name strings. `nil` uses `Rinku.skip_tags`; if
  that value is also `nil`, Rinku uses the default list above. An empty array
  disables tag skipping. Objects that include `Rinku` also have the
  `skip_tags` accessor.
* `flags`: an integer bitmask. `Rinku::AUTOLINK_SHORT_DOMAINS` allows short
  domains such as `http://intranet`.
* `&block`: an optional block. Rinku calls it with each link string. The block
  must return a `String`; Rinku uses it as the link label without escaping it.
  Return trusted or already escaped content.

For example, set an attribute on each generated link:

```ruby
Rinku.auto_link('http://www.example.com', :all, 'target="_blank"')
# => '<a href="http://www.example.com" target="_blank">http://www.example.com</a>'
```

Use a block to change the link label:

```ruby
Rinku.auto_link('Visit http://www.example.com') do |_url|
  'Example site'
end
# => 'Visit <a href="http://www.example.com">Example site</a>'
```

Rinku accepts only the stated Ruby types. It does not call `to_str`, `to_int`,
or `to_ary` to convert arguments. It scans bytes and does not require valid
UTF-8. It keeps unchanged input bytes and sets generated output to the source's
current encoding. If Rinku finds no link, it returns the original input object,
including frozen strings and string subclasses. URL schemes are
case-insensitive. HTML tag names and `www.` are case-sensitive.

## Rails integration

Load ActionView or Rails before you require `rails_rinku`:

```ruby
require 'rails_rinku'
```

The integration replaces the removed Rails `auto_link` helper in ActionView's
`TextHelper`. It escapes unsafe input, applies HTML options, and returns safe
output. It also passes callbacks to Rinku. Rails 7.2 and later use the current
tag builder API.

## Development

The Rust binding copies Ruby string bytes into owned buffers before it uses them.
Clippy rejects APIs that borrow Ruby string storage. See the
[ownership boundary and enforcement rules](docs/rust-rewrite.md#enforcing-owned-string-access).

From the repository root, run:

```sh
bundle install
bundle exec rake test gc_stress
cargo test -p rinku-core --locked
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
rustup component add --toolchain nightly-2026-10-01 miri
cargo +nightly-2026-10-01 miri test -p rinku-core --locked
ruby tools/package_smoke.rb
```


## Releasing a new version

A new version is published to RubyGems.org every time a change to `version.rb` is pushed to the `main` branch.
In short, follow these steps:
1. Update `version.rb`,
2. run `bundle lock` to update `Gemfile.lock`,
3. merge this change into `main`, and
4. look at [the action](https://github.com/zendesk/rinku/actions/workflows/publish.yml) for output.

To create a pre-release from a non-main branch:
1. change the version in `version.rb` to something like `1.2.0.pre.1` or `2.0.0.beta.2`,
2. push this change to your branch,
3. go to [Actions → “Publish to RubyGems.org” on GitHub](https://github.com/zendesk/rinku/actions/workflows/publish.yml),
4. click the “Run workflow” button,
5. pick your branch from a dropdown.

## License

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted, provided that the above
copyright notice and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES
WITH REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF
MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR
ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES
WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN
ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF
OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS SOFTWARE.
