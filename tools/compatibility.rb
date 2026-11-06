# frozen_string_literal: true
require 'base64'

module Compatibility
  class StringChild < String; end
  class Coercible
    def to_str = 'https://example.com'
    def to_int = 1
    def to_ary = ['pre']
  end

  def self.string(value)
    { bytes: Base64.strict_encode64(value.b), encoding: value.encoding.name,
      class: value.class.name, frozen: value.frozen?, valid_encoding: value.valid_encoding? }
  end

  def self.observe(args, callback = nil, receiver = Rinku)
    trace = []
    result = receiver.auto_link(*args) do |link|
      trace << string(link)
      callback ? callback.call(link) : link
    end
    observation = { result: string(result), identical: result.equal?(args.first), callbacks: trace }
    unless callback
      plain = receiver.auto_link(*args)
      observation[:without_block] = { result: string(plain), identical: plain.equal?(args.first) }
    end
    observation
  rescue Exception => e # Record errors that do not inherit from StandardError too.
    { exception: e.class.name, message: e.message, callbacks: trace }
  end

  def self.cases
    text = 'http://example.com www.example.org a@b.com'
    cases = {}
    add = ->(name, args, callback = nil) { cases[name] = observe(args, callback) }
    [[], [text], [text, nil], [text, :all], [text, :urls], [text, :email_addresses],
      [text, :all, '  rel="x"'], [text, :all, ''], [text, nil, "a\0b"],
      [text, nil, nil, []], [text, nil, nil, nil, 0], [text, nil, nil, nil, -1],
      [text, nil, nil, nil, 2**32 - 1], [text, nil, nil, nil, 2**32],
      [text, nil, nil, nil, -(2**31) - 1], [text, nil, nil, nil, 2**70],
      [text, nil, nil, nil, 1, nil]].each_with_index { |args, i| add.call("arguments/#{i}", args) }
    bad = [nil, true, false, 1, 1.0, :all, [], {}, Coercible.new]
    bad.each_with_index do |value, i|
      5.times do |position|
        args = [text, nil, nil, nil, nil]
        args[position] = value
        add.call("types/#{position}/#{i}", args)
      end
    end
    [[1, 1, 1, 1, true], [text, 1, 1, 1, true], [text, :bad, 1, 1, true],
      [text, :bad, nil, [1], true], [text, :bad, nil, [], true],
      [text, :bad, nil, [], 0], [text, nil, nil, ["a\0b"]],
      [text, nil, nil, ['pre'.encode('UTF-16LE')]]].each_with_index do |args, i|
      add.call("validation_order/#{i}", args)
    end
    ['', ''.freeze, 'no links'.freeze, StringChild.new('plain'), StringChild.new(text),
      "\xff http://example.com/\xfe".b, "http://example.com/\0tail".b,
      +"http://example.com/\xff", text.encode('UTF-16LE'),
      text.encode('ISO-8859-1')].each_with_index { |input, i| add.call("bytes/#{i}", [input]) }
    ['HTTP://EXAMPLE.COM WWW.example.com www.example.com', 'xwww.a.com (www.a.com)',
      '<PRE>http://example.com</PRE> <pre>http://example.com</pre>',
      '<pre/>http://example.com <pre>http://example.com',
      '<div title="http://example.com">http://example.org</div>',
      '<a><a>http://example.com</a>http://example.org</a>',
      '< malformed http://example.com', 'http://x http://x. http://.x.com http://-x.com',
      'a+b@d.com+e@f.com', 'a@b.c a@b.c. a@b.c..', 'a@b@c.com later@d.com',
      'http://a.com?<br>http://b.com', 'http://a.com/<',
      'www.a.com&amp;<code>http://b.com</code>www.c.com',
      'www.www.éhttp://a.com <pre>www.hidden.com</pre>www.b.com a@c.com',
      "www.www.com\u200cwww.b.com", 'www.www.com/é', 'www.www.',
      "www.www.\xffa@b.com".b,
      '&lt;http://www.google.com&gt;)&lt;)&lt;)&lt;)&lt;)&lt;)&lt;)'
    ].each_with_index { |input, i| add.call("parser/#{i}", [input]) }
    [0, 1, 2, -1].each { |flag| add.call("flags/#{flag}", ['http://x http://xy', nil, nil, nil, flag]) }
    ([*0x2000..0x200d, 0x2028, 0x2029, 0x202f, 0x205f, 0x3000, 0xfeff, 0xa0, 0x85] + (0..127).to_a).each do |code|
      add.call("whitespace/#{code}", ["http://example.com/path#{code.chr(Encoding::UTF_8)}end"])
    end
    add.call('callback/reverse', [text], ->(link) { link.reverse })
    (0..255).each do |byte|
      add.call("www_prefix_byte/#{byte}", [byte.chr(Encoding::BINARY) + 'www.a.com'.b])
    end
    add.call('callback/binary', [text], ->(_) { "\xff\0".b })
    add.call('callback/subclass', [text], ->(link) { StringChild.new(link) })
    add.call('callback/coercion', [text], ->(_) { Coercible.new })
    add.call('callback/nil', [text], ->(_) { nil })
    add.call('callback/exception', [text], ->(_) { raise ArgumentError, 'callback failed' })
    source = +'http://a.com http://b.com'
    attributes = +'rel=x'
    add.call('mutation/same_length_source_and_attributes', [source, nil, attributes], ->(link) {
      source.setbyte(20, 'c'.ord)
      attributes.setbyte(4, 'y'.ord)
      link
    })
    tag = +'pre'
    add.call('mutation/tag_string', ['http://a.com <pre>http://b.com</pre>', nil, nil, [tag]], ->(link) {
      tag.setbyte(0, 'x'.ord)
      link
    })
    tags = [+'pre']
    add.call('mutation/tag_array', ['http://a.com <pre>http://b.com</pre>', nil, nil, tags], ->(link) {
      tags[0] = 'div'
      link
    })
    # Only in-place, same-length edits belong in the C oracle. Resizing can
    # invalidate C's retained pointers and is covered by Rust-only Ruby tests.
    source = +'before http://a.com after'
    add.call('mutation/emitted_current_and_unread_bytes', [source], ->(link) {
      source[0, 6] = 'BEFORE'
      source.setbyte(source.index('a.com'), 'x'.ord)
      source[-5, 5] = 'AFTER'
      link
    })
    attributes = +'  rel=x'
    calls = 0
    add.call('mutation/attribute_whitespace_offset', [text, nil, attributes], ->(link) {
      calls += 1
      if calls == 1
        attributes.setbyte(1, 'z'.ord)
        attributes.setbyte(6, 'y'.ord)
      end
      link
    })
    attributes = +'rel=x more=y'
    tag = +'prex'
    calls = 0
    add.call('mutation/introduced_nul', ['http://a.com <pre>http://b.com</pre> http://c.com', nil, attributes, [tag]], ->(link) {
      calls += 1
      if calls == 1
        attributes.setbyte(5, 0)
        tag.setbyte(3, 0)
      end
      link
    })
    result = +'one'
    labels = %w[one two tri]
    calls = 0
    add.call('callback/reused_mutable_result', [text], ->(_) {
      labels.fetch(calls).bytes.each_with_index { |byte, i| result.setbyte(i, byte) }
      calls += 1
      result
    })
    cases['callback/nonlocal_throw'] = { result: catch(:done) {
      Rinku.auto_link(text) { throw :done, 'thrown' }
    } }
    custom_proc = ->(_) { 'body' }
    def custom_proc.call(link) = "override:#{link}"
    cases['callback/proc_call_override'] = string(Rinku.auto_link('http://a.com', &custom_proc))
    begin
      Rinku.skip_tags = ['div']
      add.call('skip/global', ['<div>http://example.com</div> <pre>http://example.com</pre>'])
      add.call('skip/custom', ['<div>http://example.com</div> <pre>http://example.com</pre>', nil, nil, []])
      receiver = Class.new { include Rinku }.new
      cases['inclusion/default'] = observe([text], nil, receiver)
      receiver.skip_tags = ['div']
      cases['inclusion/skip'] = observe(['<div>http://example.com</div>'], nil, receiver)
    ensure
      Rinku.skip_tags = nil
    end
    cases.merge(scanner_cases)
  end

  def self.scanner_cases
    cases = {}
    [:all, :urls, :email_addresses].each do |mode|
      %w[HtTp://a.com hTtPs://a.com FtP://a.com mailto://a.com httpx://a.com xhttp://a.com].each do |input|
        cases["scanner/scheme/#{mode}/#{input}"] = observe([input, mode])
      end
      [6, 7, 8].each do |passes|
        [')' * passes, '&lt;)' * (passes - 1)].each_with_index do |suffix, i|
          cases["scanner/trim/#{mode}/#{passes}/#{i}"] = observe(['http://a.com' + suffix, mode])
        end
      end
      # Offsets refer to the active byte itself, not to the token or padding.
      first_group = [['<pre>http://hidden.com</pre>', 0], ['www.a.com', 0], ['WWW.a.com', 0]]
      second_group = [['http://b.com', 4], ['a@b.com', 1]]
      [255, 256, 257].each do |offset|
        first_group.product(second_group).each_with_index do |pair, i|
          [pair, pair.reverse].each_with_index do |(first, second), order|
            input = ' ' * (offset - first[1]) + first[0] + ' ' + second[0]
            raise 'incorrect active offset' unless input.getbyte(offset) == first[0].getbyte(first[1])
            cases["scanner/boundary/#{mode}/#{offset}/#{i}/#{order}"] = observe([input, mode])
          end
        end
      end
    end
    cases
  end

  def self.random_cases
    random = Random.new(918_273)
    atoms = ['plain', 'http://a.com/path', 'https://b.com?q=x&amp;y=z', 'www.a.com',
      'a+b@c.com', '<pre>', '</pre>', '<div x="http://a.com">', '</div>',
      '<a>', '</a>', '&lt;', '&gt;', ';', '(', ')', ']', '?', '!', ' ', "\n", "\0"]
    Array.new(512) do |i|
      input = Array.new(random.rand(1..16)) { atoms.sample(random: random) }.join
      args = [input, [:all, :urls, :email_addresses].sample(random: random), nil,
        [nil, [], ['div'], ['a', 'pre']].sample(random: random), random.rand(2)]
      ["random/#{i}", observe(args)]
    end.to_h
  end
end
