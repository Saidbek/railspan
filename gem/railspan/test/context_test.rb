# frozen_string_literal: true

require_relative "test_helper"

class ContextTest < Minitest::Test
  def setup
    setup_railspan
    Railspan::Context.clear!
  end

  def test_nested_spans_parent_ids
    parent = Railspan::Tracer.start_span(name: "root", kind: "http.server", resource: "GET /")
    child = Railspan::Tracer.start_span(name: "sql", kind: "sql", resource: "SELECT 1")

    assert_equal parent.trace_id, child.trace_id
    assert_equal parent.span_id, child.parent_span_id
    assert_nil parent.parent_span_id

    Railspan::Tracer.finish_span(child)
    Railspan::Tracer.finish_span(parent)

    assert_nil Railspan::Context.current
    assert_equal 2, Railspan.exporter.spans.size
  end

  def test_clear_after_request
    Railspan::Tracer.start_span(name: "root", kind: "http.server")
    Railspan::Context.clear!
    assert_nil Railspan::Context.current
  end

  def test_in_span_handles_errors
    assert_raises(RuntimeError) do
      Railspan::Tracer.in_span(name: "boom", kind: "custom") do
        raise "fail"
      end
    end
    span = Railspan.exporter.spans.last
    assert_equal "error", span["status"]
    assert_equal true, span["attributes"]["error"]
  end
end
