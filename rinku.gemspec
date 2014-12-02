Gem::Specification.new do |s|
  s.name = 'zendesk-rinku'
  s.version = '1.7.0'
  s.summary = "Mostly autolinking"
  s.description = <<-EOF
    A fast and very smart autolinking library that
    acts as a drop-in replacement for Rails `auto_link`

    This is just a fork of vmg's lib with a small patch we need.
  EOF
  s.email = 'shajith@zendesk.com'
  s.homepage = 'https://github.com/zendesk/rinku'
  s.authors = ["Vicent Marti", "Shajith Chacko"]
  s.license = "ISC"
  s.files = %w[
    COPYING
    README.markdown
    Rakefile
    ext/rinku/rinku.c
    ext/rinku/rinku.h
    ext/rinku/autolink.c
    ext/rinku/autolink.h
    ext/rinku/buffer.c
    ext/rinku/buffer.h
    ext/rinku/extconf.rb
    lib/rinku.rb
    lib/rails_rinku.rb
    rinku.gemspec
  ]
  s.extensions = ["ext/rinku/extconf.rb"]
  s.add_development_dependency "rake"
  s.add_development_dependency "rake-compiler"
end
