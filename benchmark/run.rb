# frozen_string_literal: true
require 'json'
require 'open3'
require 'rbconfig'
require 'fileutils'
require 'time'

abort 'usage: ruby benchmark/run.rb' unless ARGV.empty?
samples = Integer(ENV.fetch('SAMPLES', '6'))
abort 'SAMPLES must be positive' unless samples.positive?
destination = File.join(__dir__, 'results')
FileUtils.mkdir_p(destination)

runs = Array.new(samples) do |round|
  warn "Sample #{round + 1}/#{samples}"
  stdout, status = Open3.capture2(RbConfig.ruby, '--disable=yjit', File.join(__dir__, 'worker.rb'))
  abort 'Benchmark worker failed' unless status.success?
  JSON.parse(stdout)
end

rows = runs.first.fetch('measurements').each_with_index.map do |test, i|
  measurements = runs.map { |run| run.fetch('measurements').fetch(i) }
  seconds = measurements.sum { |row| row.fetch('seconds_per_op') } / samples
  { name: test.fetch('name'), ns_per_op: seconds * 1e9, ops_per_second: 1 / seconds,
    ruby_allocations_per_op: measurements.sum { |row| row.fetch('ruby_allocations_per_op') } / samples }
end
File.write(File.join(destination, 'raw.json'), JSON.pretty_generate(timestamp: Time.now.utc.iso8601, runs: runs))
File.write(File.join(destination, 'summary.json'), JSON.pretty_generate(cases: rows))
puts '| Workload | ns/op | ops/s | Ruby allocations/op |'
puts '|---|---:|---:|---:|'
rows.each { |row| puts format('| %s | %.1f | %.1f | %.2f |',
  row[:name], row[:ns_per_op], row[:ops_per_second], row[:ruby_allocations_per_op]) }
puts "Results: #{destination} (overwritten on each run)"
