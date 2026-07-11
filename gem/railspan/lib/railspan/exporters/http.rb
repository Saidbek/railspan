# frozen_string_literal: true

require "json"
require "net/http"
require "uri"

module Railspan
  module Exporters
    # Batches finished spans and POSTs them to the agent. Fail-open: never raises into the app.
    class Http
      def initialize(config: Railspan.config)
        @config = config
        @mutex = Mutex.new
        @buffer = []
        @dropped = 0
        @stopped = false
        @worker = Thread.new { run_loop }
        @worker.abort_on_exception = false
        @worker.report_on_exception = false if @worker.respond_to?(:report_on_exception=)
      end

      def export(span)
        return if @stopped

        @mutex.synchronize do
          if @buffer.size >= @config.max_queue_spans
            @dropped += 1
            return
          end
          @buffer << span.to_h
        end
      rescue StandardError
        # fail-open
      end

      def shutdown
        @stopped = true
        @worker.join(2)
        flush
      rescue StandardError
        nil
      end

      def dropped_count
        @mutex.synchronize { @dropped }
      end

      def flush
        batch = nil
        @mutex.synchronize do
          return if @buffer.empty?

          batch = @buffer
          @buffer = []
        end
        send_batch(batch) if batch && !batch.empty?
      end

      private

      def run_loop
        until @stopped
          sleep @config.flush_interval
          flush
        end
      rescue StandardError
        # keep thread alive until stop
        retry unless @stopped
      end

      def send_batch(spans)
        payload = {
          "protocol_version" => 1,
          "sdk" => {
            "name" => "railspan-ruby",
            "version" => Railspan::VERSION,
            "language" => "ruby",
            "runtime" => "ruby-#{RUBY_VERSION}"
          },
          "resource" => {
            "service.name" => @config.service_name,
            "deployment.environment" => @config.environment
          },
          "spans" => spans
        }

        base = @config.endpoint.to_s.sub(%r{/\z}, "")
        uri = URI.parse("#{base}/v1/traces")
        http = Net::HTTP.new(uri.host, uri.port)
        http.open_timeout = 2
        http.read_timeout = 3
        http.use_ssl = uri.scheme == "https"

        req = Net::HTTP::Post.new(uri.request_uri)
        req["Content-Type"] = "application/json"
        req["Authorization"] = "Bearer #{@config.api_key}" if @config.api_key && !@config.api_key.empty?
        req.body = JSON.generate(payload)
        http.request(req)
      rescue StandardError => e
        warn "[railspan] http export failed: #{e.class}: #{e.message}" if ENV["RAILSPAN_DEBUG"]
      end
    end
  end
end
