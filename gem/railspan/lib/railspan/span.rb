# frozen_string_literal: true

module Railspan
  class Span
    attr_reader :trace_id, :span_id, :parent_span_id, :name, :kind,
                :start_time_unix_ns, :attributes, :events
    attr_accessor :end_time_unix_ns, :status, :resource

    def initialize(trace_id:, span_id:, parent_span_id: nil, name:, kind:, resource: nil, attributes: {})
      @trace_id = trace_id
      @span_id = span_id
      @parent_span_id = parent_span_id
      @name = name
      @kind = kind
      @resource = resource
      @start_time_unix_ns = Tracer.now_ns
      @end_time_unix_ns = nil
      @status = "ok"
      @attributes = attributes.each_with_object({}) { |(k, v), h| h[k.to_s] = v }
      @events = []
    end

    def finish(status: nil, attributes: {})
      @end_time_unix_ns ||= Tracer.now_ns
      @status = status if status
      attributes.each { |k, v| @attributes[k.to_s] = v }
      self
    end

    def finished?
      !@end_time_unix_ns.nil?
    end

    def duration_ns
      return 0 unless finished?

      @end_time_unix_ns - @start_time_unix_ns
    end

    def add_event(name, attributes: {})
      @events << {
        "time_unix_ns" => Tracer.now_ns,
        "name" => name,
        "attributes" => attributes.transform_keys(&:to_s)
      }
    end

    def set_error(exception)
      @status = "error"
      @attributes["error"] = true
      @attributes["error.type"] = exception.class.name
      @attributes["error.message"] = exception.message.to_s[0, 500]
      add_event("exception", attributes: {
        "exception.type" => exception.class.name,
        "exception.message" => exception.message.to_s[0, 500],
        "exception.stacktrace" => Array(exception.backtrace).first(20).join("\n")
      })
    end

    def to_h
      {
        "trace_id" => @trace_id,
        "span_id" => @span_id,
        "parent_span_id" => @parent_span_id,
        "name" => @name,
        "kind" => @kind,
        "resource" => @resource,
        "start_time_unix_ns" => @start_time_unix_ns,
        "end_time_unix_ns" => @end_time_unix_ns || Tracer.now_ns,
        "status" => @status,
        "attributes" => @attributes,
        "events" => @events
      }
    end
  end
end
