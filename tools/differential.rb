# frozen_string_literal: true
require 'json'
require 'open3'
require 'rbconfig'
baseline = ARGV.fetch(0) { abort 'usage: ruby tools/differential.rb BASELINE_LOAD_PATH [RUST_LOAD_PATH]' }
candidate = ARGV.fetch(1, File.expand_path('../lib', __dir__))
results = [baseline, candidate].map do |path|
  output, status = Open3.capture2({ 'RINKU_LIB' => path }, RbConfig.ruby, File.join(__dir__, 'observations.rb'))
  abort "Observations failed for #{path}" unless status.success?
  JSON.parse(output)
end
differences = (results[0].keys | results[1].keys).select { |key| results[0][key] != results[1][key] }
differences.each { |key| warn "#{key}\n C: #{results[0][key].inspect}\n Rust: #{results[1][key].inspect}" }
abort "#{differences.size} differences" unless differences.empty?
puts "#{results[0].size} identical observations (including both datasets and supplemental workloads)"
