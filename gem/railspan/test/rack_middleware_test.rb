# frozen_string_literal: true

require_relative "test_helper"
require "rack"

class RackMiddlewareTest < Minitest::Test
  def setup
    setup_railspan
  end

  def test_creates_root_span
    app = ->(_env) { [200, { "content-type" => "text/plain" }, ["ok"]] }
    mw = Railspan::Middleware::Rack.new(app)
    status, = mw.call(Rack::MockRequest.env_for("/users/1", method: "GET"))

    assert_equal 200, status
    spans = Railspan.exporter.spans
    assert_equal 1, spans.size
    assert_equal "http.server", spans.first["kind"]
    assert_equal "GET /users/1", spans.first["resource"]
    assert_equal 200, spans.first["attributes"]["http.status_code"]
  end

  def test_ignores_health_path
    app = ->(_env) { [200, {}, ["ok"]] }
    mw = Railspan::Middleware::Rack.new(app)
    mw.call(Rack::MockRequest.env_for("/up"))
    assert_empty Railspan.exporter.spans
  end

  def test_marks_errors
    app = ->(_env) { raise "boom" }
    mw = Railspan::Middleware::Rack.new(app)
    assert_raises(RuntimeError) { mw.call(Rack::MockRequest.env_for("/fail")) }
    span = Railspan.exporter.spans.first
    assert_equal "error", span["status"]
  end
end
