# frozen_string_literal: true
require 'tmpdir'
require 'rbconfig'
require 'fileutils'
require 'rubygems'

root = File.expand_path('..', __dir__)
require File.join(root, 'lib/version')
gem_command = File.join(RbConfig::CONFIG.fetch('bindir'), 'gem')
Dir.mktmpdir('rinku-package-') do |dir|
  gemfile = File.join(dir, "zendesk-rinku-#{Rinku::VERSION}.gem")
  abort 'gem build failed' unless system(RbConfig.ruby, gem_command, 'build',
    File.join(root, 'rinku.gemspec'), '--output', gemfile)
  env = ENV.to_h.select { |key, _| key.start_with?('BUNDLE_') }.transform_values { nil }
  env.merge!('GEM_HOME' => File.join(dir, 'gems'), 'GEM_PATH' => Gem.path.join(File::PATH_SEPARATOR),
    'RUBYLIB' => nil, 'RUBYOPT' => nil)
  Dir.chdir(dir) do
    abort 'gem installation failed' unless system(env, RbConfig.ruby, gem_command, 'install', gemfile,
      '--no-document', '--local', '--ignore-dependencies')
    program = <<~'RUBY'
      require 'rinku'
      require 'version'
      libraries = $LOADED_FEATURES.grep(/rinku\.(so|bundle)$/)
      home = File.realpath(ENV.fetch('GEM_HOME'))
      raise "loaded outside GEM_HOME: #{libraries.inspect}" unless !libraries.empty? && libraries.all? { |path| File.realpath(path).start_with?(home + '/') }
      raise 'linking failed' unless Rinku.auto_link('http://a.com') == '<a href="http://a.com">http://a.com</a>'
      plain = 'plain'.freeze
      raise 'identity failed' unless Rinku.auto_link(plain).equal?(plain)
      seen = []
      result = Rinku.auto_link('http://a.com a@b.com') do |link|
        seen << link
        GC.start
        GC.compact if GC.respond_to?(:compact)
        link.upcase
      end
      raise 'callback trace failed' unless seen == ['http://a.com', 'a@b.com']
      raise 'callback rendering failed' unless result == '<a href="http://a.com">HTTP://A.COM</a> <a href="mailto:a@b.com">A@B.COM</a>'
      raise 'nonlocal exit failed' unless catch(:done) { Rinku.auto_link('http://a.com') { throw :done, :ok } } == :ok
      require 'action_view'
      require 'rails_rinku'
      helper = ActionView::Base.empty
      html = helper.auto_link('http://a.com', html: { rel: 'nofollow' })
      raise 'Rails failed' unless html.html_safe? && html == '<a href="http://a.com" rel="nofollow">http://a.com</a>'
      puts "Packaged #{Rinku::VERSION} loaded outside checkout"
    RUBY
    abort 'package smoke failed' unless system(env, RbConfig.ruby, '-e', program)
  end
end
