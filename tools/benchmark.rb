# frozen_string_literal: true
require 'json'
require 'open3'
require 'rbconfig'
require 'fileutils'
require 'digest'
require 'time'

baseline = ARGV.fetch(0) { abort 'usage: ruby tools/benchmark.rb BASELINE_LOAD_PATH OUTPUT_DIRECTORY [--report-only]' }
destination = File.expand_path(ARGV.fetch(1))
FileUtils.mkdir_p(destination)
candidate = ENV.fetch('RUST_RINKU_LIB', File.expand_path('../lib', __dir__))
abort 'Compatibility check failed' unless system(RbConfig.ruby, File.join(__dir__, 'differential.rb'), baseline, candidate)
samples = Integer(ENV.fetch('SAMPLES', '12'))
abort 'At least 3 independent samples are required' if samples < 3
raw = { 'c' => [], 'rust' => [] }
samples.times do |round|
  (round.even? ? %w[c rust] : %w[rust c]).each do |implementation|
    warn "Sample #{round + 1}/#{samples}: #{implementation}"
    env = { 'RINKU_LIB' => implementation == 'c' ? baseline : candidate }
    stdout, status = Open3.capture2(env, RbConfig.ruby, '--disable=yjit', File.join(__dir__, 'benchmark_worker.rb'))
    abort "#{implementation} worker failed" unless status.success?
    raw[implementation] << JSON.parse(stdout)
  end
end

def command(*args)
  output, status = Open3.capture2e(*args)
  status.success? ? output.strip : 'unavailable'
end

raw['metadata'] = {
  'timestamp' => Time.now.utc.iso8601, 'revision' => command('git', 'rev-parse', 'HEAD'),
  'worktree_diff_sha256' => Digest::SHA256.hexdigest(command('git', 'diff', 'HEAD')),
  'source_sha256' => (Dir['crates/**/*.rs', 'crates/**/Cargo.toml', 'ext/rinku/src/*.rs',
    'ext/rinku/Cargo.toml', 'ext/rinku/extconf.rb', 'lib/*.rb'] + %w[Cargo.toml Cargo.lock]).sort.to_h { |file|
      [file, Digest::SHA256.file(file).hexdigest]
    },
  'rustc' => command('rustc', '-Vv'), 'cargo' => command('cargo', '-V'),
  'cc' => command(RbConfig::CONFIG.fetch('CC'), '--version'),
  'platform' => command('uname', '-a'), 'invocation' => [RbConfig.ruby, $PROGRAM_NAME, *ARGV],
  'build' => 'rb_sys release; thin LTO; codegen-units=1; Cargo.lock pinned',
  'baseline_revision' => '67b6b61b5017a6aa6754bc3bb83dd7d95ca4c863',
  'qualification' => 'Exploratory unless run on controlled release hardware',
  'native_memory' => 'Not included in Ruby allocations; see docs/rust-rewrite.md for separate profiling'
}
File.write(File.join(destination, 'raw.json'), JSON.pretty_generate(raw))

def mean(values) = values.sum / values.length
def interval(values, random)
  estimates = Array.new(10_000) { mean(Array.new(values.length) { values[random.rand(values.length)] }) }.sort
  [estimates[249], estimates[9749]]
end

def ratio_interval(c, rust, random)
  estimates = Array.new(10_000) do
    indexes = Array.new(c.length) { random.rand(c.length) }
    indexes.sum { |i| rust[i] } / indexes.sum { |i| c[i] }
  end.sort
  [estimates[249], estimates[9749]]
end

names = raw['c'].first.fetch('measurements').map { |row| row.fetch('name') }
random = Random.new(12345)
rows = names.each_with_index.map do |name, i|
  c = raw['c'].map { |run| run.fetch('measurements')[i].fetch('seconds_per_op') }
  rust = raw['rust'].map { |run| run.fetch('measurements')[i].fetch('seconds_per_op') }
  # Paired, alternating independent processes. Resample whole pairs and take
  # the ratio of arithmetic mean times, matching the reported ns/op values.
  ratio_ci = ratio_interval(c, rust, random)
  { name: name, c_ns: mean(c) * 1e9, rust_ns: mean(rust) * 1e9,
    c_ns_ci95: interval(c, random).map { |v| v * 1e9 },
    rust_ns_ci95: interval(rust, random).map { |v| v * 1e9 },
    c_ops_per_second: 1 / mean(c), rust_ops_per_second: 1 / mean(rust),
    ratio: mean(rust) / mean(c), ratio_ci95: ratio_ci,
    c_ruby_allocations: mean(raw['c'].map { |run| run['measurements'][i]['ruby_allocations_per_op'] }),
    rust_ruby_allocations: mean(raw['rust'].map { |run| run['measurements'][i]['ruby_allocations_per_op'] }),
    gate: ratio_ci.last <= 1.0 ? 'pass' : ratio_ci.first > 1.0 ? 'regression' : 'inconclusive' }
end
aggregates = %w[benchmark mail_benchmark supplemental combined].to_h do |dataset|
  group = rows.select { |row| dataset == 'combined' || row[:name].start_with?("#{dataset}/") }
  [dataset, { geometric_mean_ratio: Math.exp(mean(group.map { |row| Math.log(row[:ratio]) })),
    definition: 'Equal weight per case; geometric mean of Rust/C time ratios. Not a release gate.' }]
end
summary = { cases: rows, aggregates: aggregates, gate_passed: rows.all? { |row| row[:gate] == 'pass' } }
File.write(File.join(destination, 'summary.json'), JSON.pretty_generate(summary))
puts '| Workload | C ns/op | Rust ns/op | Rust/C (95% CI) | Gate |'
puts '|---|---:|---:|---:|---|'
rows.each { |row| puts format('| %s | %.1f | %.1f | %.3f (%.3f–%.3f) | %s |',
  row[:name], row[:c_ns], row[:rust_ns], row[:ratio], *row[:ratio_ci95], row[:gate]) }
puts "Raw samples and metadata: #{destination}"
exit 1 unless summary[:gate_passed] || ARGV.include?('--report-only')
