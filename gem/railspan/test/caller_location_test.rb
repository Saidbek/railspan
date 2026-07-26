# frozen_string_literal: true

require_relative "test_helper"

class CallerLocationTest < Minitest::Test
  def setup
    Railspan.reset!
    Railspan.configure do |c|
      c.enabled = true
      c.exporter = :null
      c.capture_source_location = true
      c.application_root = File.expand_path("..", __dir__) # gem/railspan
    end
  end

  def teardown
    Railspan.reset!
  end

  def test_capture_finds_app_frame
    # application_root is gem/railspan; this test file lives under test/
    loc = helper_that_captures
    refute_nil loc
    assert loc["code.filepath"].include?("caller_location_test.rb"), loc.inspect
    assert_kind_of Integer, loc["code.lineno"]
    assert loc["code.lineno"] > 0
    assert loc["code.function"]
  end

  def test_disabled_returns_nil
    Railspan.config.capture_source_location = false
    assert_nil Railspan::CallerLocation.capture(skip: 1)
  end

  def test_sql_span_gets_code_attributes
    Railspan.config.application_root = File.expand_path("..", __dir__)
    span = make_sql_span_from_app_code
    assert span.attributes["code.filepath"], span.attributes.inspect
    assert span.attributes["code.filepath"].include?("caller_location_test.rb")
    assert span.attributes["code.lineno"]
  ensure
    Railspan::Context.clear!
  end

  def test_http_server_kind_skipped_by_default
    span = Railspan::Tracer.start_span(name: "http.server", kind: "http.server", resource: "GET /")
    refute span.attributes.key?("code.filepath")
  ensure
    Railspan::Tracer.finish_span(span) if span
    Railspan::Context.clear!
  end

  private

  def helper_that_captures
    Railspan::CallerLocation.capture(skip: 1)
  end

  def make_sql_span_from_app_code
    Railspan::Tracer.start_span(
      name: "sql",
      kind: "sql",
      resource: "SELECT 1",
      attributes: { "db.statement" => "SELECT 1" }
    )
  end
end
