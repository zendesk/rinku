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
  s.files = Dir.glob(%w[
    COPYING README.markdown Rakefile rinku.gemspec
    Cargo.{toml,lock}
    lib/*.rb
    {crates/rinku-core,ext/rinku}/{Cargo.toml,src/**/*.rs}
    ext/rinku/extconf.rb
  ], base: __dir__).sort
  s.extensions = ["ext/rinku/extconf.rb"]
  s.required_ruby_version = ">= 3.3"
  s.add_dependency 'rb_sys', '= 0.9.130'
end
