# frozen_string_literal: true
require 'minitest/autorun'
require 'action_view'
require 'rails_rinku'

class RailsTest < Minitest::Test
  def setup
    @helper = ActionView::Base.empty
  end

  def test_public_require_and_rails_options
    output = @helper.auto_link('Visit http://example.com', html: { rel: 'nofollow' })
    assert_equal 'Visit <a href="http://example.com" rel="nofollow">http://example.com</a>', output
    assert output.html_safe?
    assert_equal '', @helper.auto_link(nil)
    assert_equal 'a@b.com', @helper.auto_link('a@b.com', link: :urls)
  end

  def test_escaping_and_callback
    assert_equal '&lt;b&gt;<a href="http://a.com">label</a>', @helper.auto_link('<b>http://a.com') { 'label' }
  end
end
