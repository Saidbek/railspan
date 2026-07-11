# frozen_string_literal: true

require_relative "test_helper"

class ScrubberTest < Minitest::Test
  def test_redacts_sensitive_keys
    attrs = { "password" => "secret", "user" => "bob", "api_key" => "xyz" }
    out = Railspan::Scrubber.scrub_attributes(attrs, keys: %w[password api_key])
    assert_equal "[REDACTED]", out["password"]
    assert_equal "[REDACTED]", out["api_key"]
    assert_equal "bob", out["user"]
  end
end
