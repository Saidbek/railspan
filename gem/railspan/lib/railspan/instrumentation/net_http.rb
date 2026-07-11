# frozen_string_literal: true

module Railspan
  module Instrumentation
    module NetHttp
      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::Net::HTTP)

        @installed = true
        ::Net::HTTP.class_eval do
          alias_method :request_without_railspan, :request

          def request(req, body = nil, &block)
            return request_without_railspan(req, body, &block) unless Railspan.config.enabled?
            return request_without_railspan(req, body, &block) unless Railspan::Context.current

            host = address
            method = req.method
            path = req.path.to_s.split("?").first
            Railspan::Tracer.in_span(
              name: "http.client",
              kind: "http.client",
              resource: "#{method} #{host}#{path}",
              attributes: {
                "http.method" => method,
                "server.address" => host,
                "url.path" => path
              }
            ) do |span|
              res = request_without_railspan(req, body, &block)
              span.attributes["http.status_code"] = res.code.to_i
              span.status = "error" if res.code.to_i >= 500
              res
            end
          end
        end
      end
    end
  end
end
