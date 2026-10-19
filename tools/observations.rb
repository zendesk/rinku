# frozen_string_literal: true
$LOAD_PATH.unshift(*ENV.fetch('RINKU_LIB', File.expand_path('../lib', __dir__)).split(File::PATH_SEPARATOR))
require 'rinku'
require_relative 'compatibility'
require_relative '../benchmark/workloads'
observations = Compatibility.cases.merge(Compatibility.random_cases)
Workloads.load(supplemental: true).each do |test|
  test.verify!
  observations[test.name] = Compatibility.string(test.run)
end
puts JSON.pretty_generate(observations)
