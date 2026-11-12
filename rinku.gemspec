require_relative "lib/version"

Gem::Specification.new do |s|
  s.name = 'zendesk-rinku'
  s.version = Rinku::VERSION
  s.summary = "Autolink URLs and email addresses"
  s.description = <<-EOF
    Autolink URLs and email addresses in Ruby strings.
    Includes a Rails ActionView integration.
  EOF
  s.email = 'shajith@zendesk.com'
  s.homepage = 'https://github.com/zendesk/rinku'
  s.authors = ["Shajith Chacko", "Jonathan Cheatham", "Matt Barker"]
  s.license = "ISC"
  s.files = %w[
    COPYING
    README.markdown
    docs/rust-rewrite.md
    Rakefile
    Cargo.toml
    Cargo.lock
    crates/rinku-core/Cargo.toml
    crates/rinku-core/src/lib.rs
    crates/rinku-core/src/autolink.rs
    crates/rinku-core/src/html.rs
    crates/rinku-core/src/render.rs
    crates/rinku-core/tests/core.rs
    ext/rinku/Cargo.toml
    ext/rinku/src/lib.rs
    ext/rinku/extconf.rb
    lib/rinku.rb
    lib/rails_rinku.rb
    lib/version.rb
    rinku.gemspec
  ]
  s.extensions = ["ext/rinku/extconf.rb"]
  s.required_ruby_version = ">= 3.3"
  s.add_dependency 'rb_sys', '= 0.9.130'
end
