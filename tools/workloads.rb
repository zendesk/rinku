# frozen_string_literal: true
require 'json'

module Workloads
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
    root = File.expand_path('..', __dir__)
    general = JSON.parse(File.read(File.join(root, 'benchmark.json'))).map do |row|
      Case.new(name: "benchmark/#{row.fetch('name')}", input: row.fetch('input'),
        expected: row.fetch('expected_output'), operation: ->(text) {
          Rinku.auto_link(Rinku.auto_link(text, :urls, 'rel="noreferrer"'), :email_addresses)
        })
    end
    mail = JSON.parse(File.read(File.join(root, 'mail_benchmark.json')))
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
        # Supplemental expected outputs are independently checked by differential.rb.
        cases << Case.new(name: "supplemental/#{name}", input: input,
          expected: operation.call(input), operation: operation)
      end
    end
    cases
  end
end
