# frozen_string_literal: true
require 'fileutils'
require 'open3'
require 'rbconfig'
require 'json'

REVISION = '67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863'
destination = File.expand_path(ARGV.fetch(0))
abort 'Destination must not exist' if File.exist?(destination)
FileUtils.mkdir_p(destination)
archive, status = Open3.capture2('git', 'archive', REVISION, 'ext', 'lib')
abort 'git archive failed' unless status.success?
Open3.popen3('tar', '-x', '-C', destination) do |input, _out, error, process|
  input.binmode.write(archive)
  input.close
  abort error.read unless process.value.success?
end
Dir.chdir(File.join(destination, 'ext/rinku')) do
  abort 'extconf failed' unless system(RbConfig.ruby, 'extconf.rb')
  abort 'make failed' unless system('make')
end
File.write(File.join(destination, 'baseline.json'), JSON.pretty_generate(
  revision: REVISION, ruby: RUBY_DESCRIPTION, ruby_executable: RbConfig.ruby,
  configure_args: RbConfig::CONFIG['configure_args'], cflags: RbConfig::CONFIG['CFLAGS']
))
puts "RINKU_LIB=#{destination}/lib#{File::PATH_SEPARATOR}#{destination}/ext/rinku"
