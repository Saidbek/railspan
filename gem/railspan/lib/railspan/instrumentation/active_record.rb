# frozen_string_literal: true

module Railspan
  module Instrumentation
    module ActiveRecord
      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::ActiveSupport::Notifications)

        @installed = true

        ::ActiveSupport::Notifications.subscribe("sql.active_record") do |*args|
          event = active_support_event(args)
          next unless Railspan.config.enabled?
          next unless Context.current

          payload = event.payload
          name = payload[:name].to_s
          next if name == "SCHEMA" || name == "TRANSACTION"
          next if payload[:sql].to_s.match?(/\A\s*(BEGIN|COMMIT|ROLLBACK|SAVEPOINT|RELEASE)/i)

          sql = payload[:sql].to_s
          fingerprint = SqlNormalizer.normalize(sql)
          duration_ms = event.duration

          span = Tracer.start_span(
            name: "sql",
            kind: "sql",
            resource: fingerprint,
            attributes: {
              "db.system" => payload[:connection]&.class&.name,
              "db.statement" => fingerprint,
              "db.operation" => name
            }.compact
          )
          # Notifications fire after completion; backdate duration
          end_ns = Tracer.now_ns
          start_ns = end_ns - (duration_ms * 1_000_000).to_i
          span.instance_variable_set(:@start_time_unix_ns, start_ns)
          span.finish
          span.instance_variable_set(:@end_time_unix_ns, end_ns)
          Context.pop if Context.current.equal?(span)
          scrubbed = Scrubber.scrub_attributes(span.attributes, keys: Railspan.config.scrub_keys)
          span.attributes.replace(scrubbed)
          Railspan.exporter&.export(span)
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
