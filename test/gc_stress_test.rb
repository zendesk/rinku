# frozen_string_literal: true
require 'minitest/autorun'
require 'rinku'

class GCStressTest < Minitest::Test
  def stressed
    previous = GC.stress
    GC.stress = true
    yield
  ensure
    GC.stress = previous
    Rinku.skip_tags = nil
  end

  def test_temporary_arguments_short_and_heap_strings
    output = stressed do
      Rinku.auto_link(('x' * 4096) + ' http://a.com <div>http://b.com</div>', :urls,
        ['rel', '="x"'].join, [%w[d i v].join])
    end
    assert_equal(('x' * 4096) + ' <a href="http://a.com" rel="x">http://a.com</a> <div>http://b.com</div>', output)
    assert_equal '<a href="http://a.co">http://a.co</a>', stressed { Rinku.auto_link(+'http://a.co') }
  end

  def test_allocations_gc_compaction_recursion_and_encoding
    trace = []
    output = stressed do
      Rinku.auto_link("http://a.com/\xff www.b.com a@b.com".b) do |link|
        16.times { 'temporary' * 32 }
        GC.start
        if GC.respond_to?(:verify_compaction_references)
          GC.verify_compaction_references(double_heap: true, toward: :empty)
        elsif GC.respond_to?(:compact)
          GC.compact
        end
        trace << [link.b, link.encoding]
        nested = Rinku.auto_link('http://nested.com') { |url| url.upcase }
        raise 'recursive call corrupted' unless nested.include?('HTTP://NESTED.COM')
        link.reverse
      end
    end
    assert_equal ["http://a.com/\xff".b, 'www.b.com'.b, 'a@b.com'.b], trace.map(&:first)
    assert trace.all? { |_, encoding| encoding == Encoding::BINARY }
    assert_equal Encoding::BINARY, output.encoding
    assert_equal "<a href=\"http://a.com/\xff\">\xff/moc.a//:ptth</a> <a href=\"http://www.b.com\">moc.b.www</a> <a href=\"mailto:a@b.com\">moc.b@a</a>".b, output
  end

  def test_mutations_and_tag_roots
    source = +'http://a.com http://b.com <pre>http://d.com</pre>'
    attributes = +'rel=x'
    tag = +'pre'
    tags = [tag]
    output = stressed do
      Rinku.auto_link(source, nil, attributes, tags) do |link|
        source.setbyte(20, 'c'.ord)
        attributes.setbyte(4, 'y'.ord)
        tag.setbyte(0, 'x'.ord)
        tags.clear
        GC.start
        link
      end
    end
    assert_equal '<a href="http://a.com" rel=x>http://a.com</a> <a href="http://c.com" rel=y>http://c.com</a> <pre><a href="http://d.com" rel=y>http://d.com</a></pre>', output
  end

  def test_original_tag_objects_survive_removal_from_caller_array
    tags = [%w[p r e].join]
    output = stressed do
      Rinku.auto_link(+'http://a.com <pre>http://b.com</pre> http://c.com', nil,
        ['rel', '=x'].join, tags) do |link|
        tags.clear
        Rinku.skip_tags = [%w[d i v].join]
        GC.start
        GC.compact if GC.respond_to?(:compact)
        link
      end
    end
    assert_equal '<a href="http://a.com" rel=x>http://a.com</a> <pre>http://b.com</pre> <a href="http://c.com" rel=x>http://c.com</a>', output
  end

  def test_safe_resize_policy_and_callback_result_copy
    source = +'http://a.com http://b.com'
    result = +"\xff\0"
    output = stressed do
      Rinku.auto_link(source) do |_link|
        source.replace('short')
        GC.start
        result
      end
    end
    assert_equal "<a href=\"http://a.com\">\xff\0</a> <a href=\"http://b.com\">\xff\0</a>".b, output.b
    assert_equal Encoding::UTF_8, output.encoding
  end

  def nonlocal_return
    Rinku.auto_link('http://a.com') { return :returned }
    :unreachable
  end

  def test_encoding_mutation_uses_initial_callback_and_current_output_encoding
    input = +'http://a.com http://b.com'
    encodings = []
    output = stressed do
      Rinku.auto_link(input) do |link|
        encodings << link.encoding
        input.force_encoding(Encoding::BINARY)
        GC.start
        link
      end
    end
    assert_equal [Encoding::UTF_8, Encoding::UTF_8], encodings
    assert_equal Encoding::BINARY, output.encoding
    assert_equal '<a href="http://a.com">http://a.com</a> <a href="http://b.com">http://b.com</a>'.b, output
  end

  def test_failure_paths_and_nonlocal_exits
    assert_raises(ArgumentError) { stressed { Rinku.auto_link('http://a.com') { GC.start; raise ArgumentError, 'callback' } } }
    assert_raises(TypeError) { stressed { Rinku.auto_link('http://a.com') { GC.start; Object.new } } }
    assert_equal :thrown, stressed { catch(:done) { Rinku.auto_link('http://a.com') { throw :done, :thrown } } }
    assert_equal :broken, stressed { Rinku.auto_link('http://a.com') { break :broken } }
    assert_equal :returned, stressed { nonlocal_return }
    assert_raises(TypeError) { stressed { Rinku.auto_link('text', nil, nil, [Object.new]) } }
  end

  def test_empty_and_no_link_identity
    inputs = ['', ''.freeze, 'no links'.freeze, 'plain'.b]
    outputs = stressed { inputs.map { |input| 3.times.map { Rinku.auto_link(input) } } }
    outputs.each_with_index { |values, i| values.each { |value| assert_same inputs[i], value } }
  end
end
