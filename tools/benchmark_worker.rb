# frozen_string_literal: true
$LOAD_PATH.unshift(*ENV.fetch('RINKU_LIB').split(File::PATH_SEPARATOR))
require 'rinku'
require 'rbconfig'
require 'open3'
require 'digest'
require_relative 'workloads'

def clock
  Process.clock_gettime(Process::CLOCK_MONOTONIC)
end

cases = Workloads.load(supplemental: true)
cases.each(&:verify!)
duration = Float(ENV.fetch('SAMPLE_SECONDS', '0.05'))
warmup = Float(ENV.fetch('WARMUP_SECONDS', '0.02'))
measurements = cases.map do |test|
  start = clock
  iterations = 0
  begin
    16.times { test.run }
    iterations += 16
  end while clock - start < warmup
  per_op = (clock - start) / iterations
  iterations = [(duration / per_op).ceil, 16].max
  GC.start
  start = clock
  iterations.times { test.run }
  elapsed = clock - start
  # Ruby object counts exclude Rust buffers. Profile native memory separately.
  GC.start
  before = GC.stat(:total_allocated_objects)
  128.times { test.run }
  allocated = GC.stat(:total_allocated_objects) - before
  { name: test.name, iterations: iterations, elapsed_seconds: elapsed,
    seconds_per_op: elapsed / iterations, ops_per_second: iterations / elapsed,
    ruby_allocations_per_op: allocated / 128.0 }
end

metadata = {
  ruby: RUBY_DESCRIPTION, executable: RbConfig.ruby, platform: RUBY_PLATFORM,
  yjit: defined?(RubyVM::YJIT) && RubyVM::YJIT.enabled?, gc_stress: GC.stress,
  gc_environment: ENV.select { |key, _| key.start_with?('RUBY_GC_') },
  native_library: $LOADED_FEATURES.grep(/rinku\.(so|bundle)$/),
  native_sha256: $LOADED_FEATURES.grep(/rinku\.(so|bundle)$/).to_h { |file| [file, Digest::SHA256.file(file).hexdigest] },
  sample_seconds: duration, warmup_seconds: warmup,
  fixture_sha256: Workloads::FIXTURE_PATHS.to_h { |file|
    [File.basename(file), Digest::SHA256.file(file).hexdigest]
  }
}
puts JSON.generate(metadata: metadata, measurements: measurements)
