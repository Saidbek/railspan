# frozen_string_literal: true

module Railspan
  class Configuration
    ATTRS = %i[
      enabled
      service_name
      environment
      endpoint
      api_key
      exporter
      sample_rate
      slow_ms
      flush_interval
      max_spans_per_batch
      max_queue_spans
      scrub_keys
      ignore_paths
      capture_source_location
      source_location_kinds
      application_root
    ].freeze

    attr_accessor(*ATTRS)

    DEFAULT_SOURCE_KINDS = %w[sql cache http.client custom].freeze

    def initialize
      @enabled = true
      @service_name = "rails-app"
      @environment = "development"
      @endpoint = "http://127.0.0.1:7421"
      @api_key = nil
      @exporter = :http
      @sample_rate = 1.0
      @slow_ms = 500
      @flush_interval = 0.2
      @max_spans_per_batch = 100
      @max_queue_spans = 10_000
      @scrub_keys = %w[
        password passwd secret token api_key access_token
        authorization auth_token credit_card ssn
      ]
      @ignore_paths = [%r{\A/up\z}, %r{\A/health\z}, %r{\A/healthz\z}, %r{\A/assets/}]
      @capture_source_location = true
      @source_location_kinds = DEFAULT_SOURCE_KINDS.dup
      @application_root = nil
      apply_env!
    end

    def apply_env!
      if (v = ENV["RAILSPAN_ENABLED"])
        @enabled = !%w[0 false no off].include?(v.strip.downcase)
      end
      @service_name = ENV["RAILSPAN_SERVICE_NAME"] if ENV["RAILSPAN_SERVICE_NAME"]
      @environment = ENV["RAILSPAN_ENVIRONMENT"] if ENV["RAILSPAN_ENVIRONMENT"]
      @endpoint = ENV["RAILSPAN_ENDPOINT"] if ENV["RAILSPAN_ENDPOINT"]
      @api_key = ENV["RAILSPAN_API_KEY"] if ENV["RAILSPAN_API_KEY"]
      if (v = ENV["RAILSPAN_EXPORTER"])
        @exporter = v.strip.downcase.to_sym
      end
      if (v = ENV["RAILSPAN_SAMPLE_RATE"])
        @sample_rate = Float(v)
      end
      if (v = ENV["RAILSPAN_SLOW_MS"])
        @slow_ms = Integer(v)
      end
      if (v = ENV["RAILSPAN_CAPTURE_SOURCE_LOCATION"])
        @capture_source_location = !%w[0 false no off].include?(v.strip.downcase)
      end
      if (v = ENV["RAILSPAN_SOURCE_LOCATION_KINDS"])
        @source_location_kinds = v.split(",").map { |s| s.strip.downcase }.reject(&:empty?)
      end
      if (v = ENV["RAILSPAN_APPLICATION_ROOT"])
        @application_root = v.strip
        @application_root = nil if @application_root.empty?
      end
    end

    def capture_source_location?
      !!@capture_source_location
    end

    def source_location_for_kind?(kind)
      return false unless capture_source_location?

      kinds = Array(@source_location_kinds).map { |k| k.to_s.downcase }
      kinds.include?(kind.to_s.downcase)
    end

    def enabled?
      !!@enabled
    end
  end
end
