# frozen_string_literal: true

require "securerandom"

module Railspan
  module Tracer
    module_function

    def now_ns
      Process.clock_gettime(Process::CLOCK_REALTIME, :nanosecond)
    rescue StandardError
      (Time.now.to_r * 1_000_000_000).to_i
    end

    def generate_trace_id
      SecureRandom.hex(16)
    end

    def generate_span_id
      SecureRandom.hex(8)
    end

    def start_span(name:, kind:, resource: nil, attributes: {}, trace_id: nil, parent: :auto)
      return NullSpan.instance unless Railspan.config.enabled?

      parent_span = parent == :auto ? Context.current : parent
      tid = trace_id || parent_span&.trace_id || generate_trace_id
      parent_id = parent_span&.span_id

      span = Span.new(
        trace_id: tid,
        span_id: generate_span_id,
        parent_span_id: parent_id,
        name: name,
        kind: kind,
        resource: resource,
        attributes: attributes
      )
      Context.push(span)
      span
    end

    def finish_span(span, status: nil, attributes: {})
      return if span.nil? || span.is_a?(NullSpan)
      return span if span.finished?

      span.finish(status: status, attributes: attributes)
      Context.pop if Context.current.equal?(span)

      scrubbed = Scrubber.scrub_attributes(span.attributes, keys: Railspan.config.scrub_keys)
      span.attributes.replace(scrubbed)
      Railspan.exporter&.export(span)
      span
    end

    def in_span(name:, kind:, resource: nil, attributes: {})
      span = start_span(name: name, kind: kind, resource: resource, attributes: attributes)
      begin
        yield span
      rescue StandardError => e
        span.set_error(e) unless span.is_a?(NullSpan)
        raise
      ensure
        finish_span(span)
      end
    end

    # No-op span when disabled
    class NullSpan
      def self.instance
        @instance ||= new
      end

      def trace_id = nil
      def span_id = nil
      def parent_span_id = nil
      def finish(*) = self
      def finished? = true
      def set_error(*) = self
      def add_event(*) = nil
      def attributes = {}
      def to_h = {}
    end
  end
end
