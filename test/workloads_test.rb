# frozen_string_literal: true
require 'minitest/autorun'
require 'rinku'
require_relative '../tools/workloads'

class WorkloadsTest < Minitest::Test
  Workloads.load(supplemental: true).each do |test|
    define_method("test_#{test.name.tr('/', '_')}") do
      assert_equal test.expected, test.run
    end
  end
end
