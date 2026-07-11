# frozen_string_literal: true

module Railspan
  module Instrumentation
    module ActionView
      EVENTS = {
        "render_template.action_view" => "view",
        "render_partial.action_view" => "partial",
        "render_collection.action_view" => "partial"
      }.freeze

      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::ActiveSupport::Notifications)

        @installed = true

        EVENTS.each do |event_name, kind|
          ::ActiveSupport::Notifications.subscribe(event_name) do |*args|
            event = active_support_event(args)
            next unless Railspan.config.enabled?
            next unless Context.current

            payload = event.payload
            identifier = payload[:identifier] || payload[:partial] || "unknown"
            resource = identifier.to_s.split("/").last(2).join("/")
            duration_ms = event.duration

            span = Tracer.start_span(
              name: kind,
              kind: kind,
              resource: resource,
              attributes: {
                "view.identifier" => identifier.to_s
              }
            )
            end_ns = Tracer.now_ns
            start_ns = end_ns - (duration_ms * 1_000_000).to_i
            span.instance_variable_set(:@start_time_unix_ns, start_ns)
            span.finish
            span.instance_variable_set(:@end_time_unix_ns, end_ns)
            Context.pop if Context.current.equal?(span)
            Railspan.exporter&.export(span)
          end
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
