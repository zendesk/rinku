# frozen_string_literal: true
$LOAD_PATH.unshift(*ENV.fetch('RINKU_LIB', File.expand_path('../lib', __dir__)).split(File::PATH_SEPARATOR))
require 'rinku'
require_relative 'workloads'
cases = Workloads.load(supplemental: true)
cases.each(&:verify!)
iterations = Integer(ENV.fetch('ITERATIONS', '100'))
cases.each { |test| iterations.times { test.run } }
GC.start
puts "Profiled #{iterations} operations per workload; Ruby GC enabled"
