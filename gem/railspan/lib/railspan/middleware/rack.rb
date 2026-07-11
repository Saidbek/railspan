# frozen_string_literal: true

module Railspan
  module Middleware
    class Rack
      def initialize(app)
        @app = app
      end

      def call(env)
        return @app.call(env) unless Railspan.config.enabled?
        return @app.call(env) if ignore?(env)

        method = env["REQUEST_METHOD"]
        path = env["PATH_INFO"].to_s
        resource = "#{method} #{path}"

        span = Tracer.start_span(
          name: "http.server",
          kind: "http.server",
          resource: resource,
          attributes: {
            "http.method" => method,
            "url.path" => path,
            "http.scheme" => env["rack.url_scheme"]
          }
        )

        status = 500
        begin
          status, headers, body = @app.call(env)
          route = extract_route(env)
          if route
            span.resource = "#{method} #{route}"
            span.attributes["http.route"] = route
          end
          controller = env["action_controller.instance"]
          if controller
            span.attributes["code.namespace"] = controller.class.name
            if controller.respond_to?(:action_name)
              span.attributes["code.function"] = controller.action_name
              # Prefer Controller#action as resource for Rails
              span.resource = "#{controller.class.name}##{controller.action_name}"
            end
          end
          span.attributes["http.status_code"] = status
          span.status = "error" if status.to_i >= 500
          [status, headers, body]
        rescue StandardError => e
          span.set_error(e)
          raise
        ensure
          Tracer.finish_span(span)
          Context.clear!
        end
      end

      private

      def ignore?(env)
        path = env["PATH_INFO"].to_s
        Array(Railspan.config.ignore_paths).any? do |pattern|
          case pattern
          when Regexp then path.match?(pattern)
          when String then path == pattern
          else false
          end
        end
      end

      def extract_route(env)
        if (params = env["action_dispatch.request.path_parameters"])
          controller = params[:controller]
          action = params[:action]
          return "/#{controller}/#{action}" if controller && action
        end
        env["sinatra.route"]
      end
    end
  end
end
