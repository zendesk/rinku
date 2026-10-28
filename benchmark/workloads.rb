# frozen_string_literal: true
require 'json'

module Workloads
  FIXTURE_DIRECTORY = File.expand_path('fixtures', __dir__)
  FIXTURE_PATHS = %w[benchmark.json mail_benchmark.json].map { |name| File.join(FIXTURE_DIRECTORY, name) }.freeze

  Case = Struct.new(:name, :input, :expected, :operation, keyword_init: true) do
    def run
      operation.call(input)
    end

    def verify!
      actual = run
      raise "#{name}: expected #{expected.inspect}, got #{actual.inspect}" unless actual == expected
    end
  end

  def self.load(supplemental: false)
    general = JSON.parse(File.read(FIXTURE_PATHS[0])).map do |row|
      Case.new(name: "benchmark/#{row.fetch('name')}", input: row.fetch('input'),
        expected: row.fetch('expected_output'), operation: ->(text) {
          Rinku.auto_link(Rinku.auto_link(text, :urls, 'rel="noreferrer"'), :email_addresses)
        })
    end
    mail = JSON.parse(File.read(FIXTURE_PATHS[1]))
    raise 'Unknown operation' unless mail.fetch('operation') == 'Rinku.auto_link'
    declared = mail.fetch('arguments')
    args = [declared.fetch('mode').to_sym, declared.fetch('link_attributes'), declared.fetch('skip_tags')]
    cases = general + mail.fetch('cases').map do |row|
      Case.new(name: "mail_benchmark/#{row.fetch('name')}", input: row.fetch('input'),
        expected: row.fetch('expected_output'), operation: ->(text) { Rinku.auto_link(text, *args) })
    end
    if supplemental
      [
        ['no_links', 'ordinary text ' * 1024, ->(text) { Rinku.auto_link(text) }],
        ['large_input', ('https://example.com/a hello a@b.com ' * 4096), ->(text) { Rinku.auto_link(text) }],
        ['callbacks', ('https://example.com/a a@b.com ' * 32), ->(text) { Rinku.auto_link(text) { |link| link.reverse } }]
      ].each do |name, input, operation|
        cases << Case.new(name: "supplemental/#{name}", input: input,
          expected: operation.call(input), operation: operation)
      end
      [1_000, 4_000, 16_000, 64_000].each do |count|
        [false, true].each do |frozen|
          input = 'http://a.com ' * count
          input.freeze if frozen
          cases << Case.new(name: "supplemental/callbacks_#{frozen ? 'frozen' : 'mutable'}_#{count}",
            input: input, expected: '<a href="http://a.com">http://a.com</a> ' * count,
            operation: ->(text) { Rinku.auto_link(text) { |link| link } })
        end
      end
      [1_000, 4_000, 16_000].each do |count|
        input = 'a@' * count
        cases << Case.new(name: "supplemental/repeated_at_#{count}", input: input,
          expected: input, operation: ->(text) { Rinku.auto_link(text) })
        cases << Case.new(name: "supplemental/adjacent_html_#{count}", input: 'http://a.com<br>' * count,
          expected: '<a href="http://a.com">http://a.com</a><br>' * count,
          operation: ->(text) { Rinku.auto_link(text) })
        input = ('www.' * count) + 'é'
        cases << Case.new(name: "supplemental/rejected_www_#{count}", input: input,
          expected: input, operation: ->(text) { Rinku.auto_link(text) })
      end
    end
    cases
  end
end
