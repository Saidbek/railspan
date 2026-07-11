# frozen_string_literal: true

Railspan.configure do |c|
  c.service_name = "dummy_rails"
  c.environment  = Rails.env.to_s
  c.endpoint     = ENV.fetch("RAILSPAN_ENDPOINT", "http://127.0.0.1:7421")
  c.api_key      = ENV["RAILSPAN_API_KEY"]
  c.exporter     = ENV.fetch("RAILSPAN_EXPORTER", "http").to_sym
  c.enabled      = ENV.fetch("RAILSPAN_ENABLED", "true") != "false"
end
