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
        res = http.request(req)
        apply_advice!(res)
      rescue StandardError => e
        warn "[railspan] http export failed: #{e.class}: #{e.message}" if ENV["RAILSPAN_DEBUG"]
      end

      # Adopt adaptive sampling advice from the server when under load.
      def apply_advice!(res)
        return unless res.is_a?(Net::HTTPSuccess)
        return if res.body.nil? || res.body.empty?

        data = JSON.parse(res.body)
        advice = data["advice"]
        return unless advice.is_a?(Hash)

        rate = advice["sample_rate"]
        return unless rate.is_a?(Numeric)

        new_rate = rate.to_f.clamp(0.0, 1.0)
        old = @config.sample_rate.to_f
        # Only lower (or gently raise toward advice) — never jump up aggressively.
        adopted = if new_rate < old
                    new_rate
                  else
                    # Slow recovery: move halfway toward advised rate
                    old + ((new_rate - old) * 0.25)
                  end
        @config.sample_rate = adopted.clamp(0.0, 1.0)
        return unless ENV["RAILSPAN_DEBUG"] && (adopted - old).abs > 0.001

        warn "[railspan] adapted sample_rate #{old} -> #{@config.sample_rate}"
      rescue StandardError
        # fail-open: ignore bad advice payloads
      end
    end
  end
end
