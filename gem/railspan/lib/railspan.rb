# frozen_string_literal: true

require_relative "railspan/version"
require_relative "railspan/configuration"
require_relative "railspan/span"
require_relative "railspan/context"
require_relative "railspan/sql_normalizer"
require_relative "railspan/scrubber"
require_relative "railspan/tracer"
require_relative "railspan/exporters/stdout"
require_relative "railspan/exporters/http"
require_relative "railspan/middleware/rack"
require_relative "railspan/instrumentation/action_controller"
require_relative "railspan/instrumentation/active_record"
require_relative "railspan/instrumentation/action_view"

module Railspan
  class << self
    def config
      @config ||= Configuration.new
    end

    def configure
      yield config
      config.apply_env!
      setup_exporter!
      self
    end

    def exporter
      @exporter
    end

    def exporter=(exporter)
      @exporter.shutdown if @exporter.respond_to?(:shutdown) && !exporter.equal?(@exporter)
      @exporter = exporter
      @exporter_kind = :custom
    end

    def setup_exporter!
      kind = config.exporter.to_sym
      return @exporter if @exporter && @exporter_kind == kind

      @exporter.shutdown if @exporter.respond_to?(:shutdown)

      @exporter = case kind
                  when :stdout then Exporters::Stdout.new
                  when :http then Exporters::Http.new(config: config)
                  when :null, :noop then nil
                  else
                    Exporters::Http.new(config: config)
                  end
      @exporter_kind = kind
      @exporter
    end

    def reset!
      @exporter.shutdown if @exporter.respond_to?(:shutdown)
      @exporter = nil
      @exporter_kind = nil
      @config = Configuration.new
      Context.clear!
    end

    def trace(name, kind: "custom", resource: nil, attributes: {}, &block)
      Tracer.in_span(name: name, kind: kind, resource: resource || name, attributes: attributes, &block)
    end
  end
end

require_relative "railspan/railtie" if defined?(Rails::Railtie)
