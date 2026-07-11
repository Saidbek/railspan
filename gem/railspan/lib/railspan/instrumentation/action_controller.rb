# frozen_string_literal: true

module Railspan
  module Instrumentation
    module ActionController
      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::ActiveSupport::Notifications)

        @installed = true

        ::ActiveSupport::Notifications.subscribe("start_processing.action_controller") do |*args|
          event = active_support_event(args)
          next unless Railspan.config.enabled?

          payload = event.payload
          resource = "#{payload[:controller]}##{payload[:action]}"
          Tracer.start_span(
            name: "controller",
            kind: "controller",
            resource: resource,
            attributes: {
              "http.method" => payload[:method]
            }.compact
          )
        end

        ::ActiveSupport::Notifications.subscribe("process_action.action_controller") do |*args|
          event = active_support_event(args)
          next unless Railspan.config.enabled?

          span = Context.current
          next unless span && span.kind == "controller"

          payload = event.payload
          attrs = {
            "http.status_code" => payload[:status],
            "view.runtime_ms" => payload[:view_runtime],
            "db.runtime_ms" => payload[:db_runtime]
          }.compact
          status = payload[:status].to_i >= 500 ? "error" : "ok"
          if payload[:exception_object]
            span.set_error(payload[:exception_object])
          elsif payload[:exception]
            span.status = "error"
            span.attributes["error"] = true
            span.attributes["error.type"] = payload[:exception].first
            span.attributes["error.message"] = payload[:exception].last.to_s[0, 500]
          end
          Tracer.finish_span(span, status: status, attributes: attrs)
        end
      end

      def active_support_event(args)
        if args.first.is_a?(::ActiveSupport::Notifications::Event)
          args.first
        else
          ::ActiveSupport::Notifications::Event.new(*args)
        end
      end
    end
  end
end
