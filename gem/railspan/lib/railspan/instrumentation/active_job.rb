# frozen_string_literal: true

module Railspan
  module Instrumentation
    module ActiveJob
      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::ActiveSupport::Notifications)

        @installed = true

        ::ActiveSupport::Notifications.subscribe("perform_start.active_job") do |*args|
          event = as_event(args)
          next unless Railspan.config.enabled?

          job = event.payload[:job]
          next unless job

          Tracer.start_span(
            name: "job",
            kind: "job",
            resource: job.class.name,
            attributes: {
              "messaging.system" => "active_job",
              "messaging.destination" => job.queue_name,
              "job.class" => job.class.name,
              "job.provider_job_id" => job.provider_job_id
            }.compact
          )
        end

        ::ActiveSupport::Notifications.subscribe("perform.active_job") do |*args|
          event = as_event(args)
          next unless Railspan.config.enabled?

          span = Context.current
          next unless span && span.kind == "job"

          payload = event.payload
          if payload[:exception_object]
            span.set_error(payload[:exception_object])
          elsif payload[:exception]
            span.status = "error"
            span.attributes["error"] = true
            span.attributes["error.type"] = payload[:exception].first
          end
          Tracer.finish_span(span)
        end

        ::ActiveSupport::Notifications.subscribe("enqueue.active_job") do |*args|
          event = as_event(args)
          next unless Railspan.config.enabled?
          next unless Context.current

          job = event.payload[:job]
          next unless job

          duration_ms = event.duration
          span = Tracer.start_span(
            name: "job.enqueue",
            kind: "job.enqueue",
            resource: job.class.name,
            attributes: {
              "messaging.system" => "active_job",
              "messaging.destination" => job.queue_name,
              "job.class" => job.class.name
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
