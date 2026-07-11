# frozen_string_literal: true

require_relative "test_helper"

class ConfigurationTest < Minitest::Test
  def test_configure_block
    Railspan.reset!
    Railspan.configure do |c|
      c.service_name = "demo"
      c.enabled = false
      c.exporter = :null
    end
    assert_equal "demo", Railspan.config.service_name
    refute Railspan.config.enabled?
  end

  def test_env_override
    ENV["RAILSPAN_SERVICE_NAME"] = "from-env"
    Railspan.reset!
    assert_equal "from-env", Railspan.config.service_name
  ensure
    ENV.delete("RAILSPAN_SERVICE_NAME")
  end
end
