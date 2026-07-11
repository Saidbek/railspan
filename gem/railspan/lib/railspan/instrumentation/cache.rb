# frozen_string_literal: true

module Railspan
  module Instrumentation
    module Cache
      EVENTS = %w[
        cache_read.active_support
        cache_write.active_support
        cache_fetch_hit.active_support
        cache_generate.active_support
        cache_delete.active_support
      ].freeze

      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::ActiveSupport::Notifications)

        @installed = true
        EVENTS.each do |event_name|
          ::ActiveSupport::Notifications.subscribe(event_name) do |*args|
            event = as_event(args)
            next unless Railspan.config.enabled?
            next unless Context.current

            payload = event.payload
            key = payload[:key].to_s
            key = key[0, 200]
            op = event_name.split(".").first.sub("cache_", "")
            duration_ms = event.duration
            span = Tracer.start_span(
              name: "cache",
              kind: "cache",
              resource: "#{op} #{key}",
              attributes: {
                "cache.operation" => op,
                "cache.key" => key,
                "cache.hit" => payload[:hit]
              }.compact
            )
            end_ns = Tracer.now_ns
            span.instance_variable_set(:@start_time_unix_ns, end_ns - (duration_ms * 1_000_000).to_i)
            span.finish
            span.instance_variable_set(:@end_time_unix_ns, end_ns)
            Context.pop if Context.current.equal?(span)
            Railspan.exporter&.export(span)
          end
        end
      end

      def as_event(args)
        if args.first.is_a?(::ActiveSupport::Notifications::Event)
          args.first
        else
          ::ActiveSupport::Notifications::Event.new(*args)
        end
      end
    end
  end
end
