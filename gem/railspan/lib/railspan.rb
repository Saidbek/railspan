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
require_relative "railspan/instrumentation/active_job"
require_relative "railspan/instrumentation/sidekiq"
require_relative "railspan/instrumentation/cache"
require_relative "railspan/instrumentation/net_http"

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

    # Record a deploy marker on the Railspan server.
    def record_deploy!(git_sha: nil, version: nil, metadata: {})
      require "net/http"
      require "json"
      require "uri"
      base = config.endpoint.to_s.sub(%r{/\z}, "")
      uri = URI.parse("#{base}/v1/deploys")
      http = Net::HTTP.new(uri.host, uri.port)
      http.use_ssl = uri.scheme == "https"
      req = Net::HTTP::Post.new(uri.request_uri)
      req["Content-Type"] = "application/json"
      req["Authorization"] = "Bearer #{config.api_key}" if config.api_key && !config.api_key.empty?
      req.body = JSON.generate({
        "git_sha" => git_sha,
        "version" => version,
        "metadata" => metadata
      }.compact)
      http.request(req)
    rescue StandardError => e
      warn "[railspan] record_deploy failed: #{e.message}" if ENV["RAILSPAN_DEBUG"]
      nil
    end
  end
end

require_relative "railspan/railtie" if defined?(Rails::Railtie)
