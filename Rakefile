# frozen_string_literal: true
require 'bundler/setup'
require 'bundler/gem_tasks'
require 'rb_sys/extensiontask'
require 'rake/testtask'

RbSys::ExtensionTask.new('rinku', Bundler::GemHelper.gemspec) do |ext|
  ext.lib_dir = 'lib'
end

Rake::TestTask.new(:test => :compile) do |test|
  test.libs << 'lib'
  test.test_files = FileList['test/*_test.rb'].exclude('test/gc_stress_test.rb')
end

Rake::TestTask.new(:gc_stress => :compile) do |test|
  test.libs << 'lib'
  test.test_files = ['test/gc_stress_test.rb']
end

desc 'Run Ruby and Rust tests'
task :default => [:test, :core]
task :core do
  sh 'cargo test -p rinku-core --locked'
end
