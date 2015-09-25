Gem::Specification.new do |s|
  s.name = 'zendesk-rinku'
  s.version = '1.7.3'
  s.summary = "Mostly autolinking"
  s.description = <<-EOF
    A fast and very smart autolinking library that
    acts as a drop-in replacement for Rails `auto_link`

    This is now a significant departure from the base gem.
  EOF
  s.email = 'shajith@zendesk.com'
  s.homepage = 'https://github.com/zendesk/rinku'
  s.authors = ["Shajith Chacko", "Jonathan Cheatham", "Matt Barker"]
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
  s.metadata['allowed_push_host'] = "https://gem.zdsys.com/gems/"
  s.extensions = ["ext/rinku/extconf.rb"]
  s.add_development_dependency "rake"
  s.add_development_dependency "rake-compiler"
  s.add_development_dependency "private_gem"
end
