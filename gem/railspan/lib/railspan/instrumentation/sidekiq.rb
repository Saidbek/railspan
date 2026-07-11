# frozen_string_literal: true

module Railspan
  module Instrumentation
    module Sidekiq
      class ServerMiddleware
        def call(worker, job, queue)
          return yield unless Railspan.config.enabled?

          class_name = job["class"] || worker.class.name
          Railspan::Tracer.in_span(
            name: "job",
            kind: "job",
            resource: class_name,
            attributes: {
              "messaging.system" => "sidekiq",
              "messaging.destination" => queue || job["queue"],
              "job.class" => class_name,
              "job.provider_job_id" => job["jid"],
              "job.attempts" => job["retry_count"] || job["retry"]
            }.compact
          ) do
            yield
          end
        end
      end

      class ClientMiddleware
        def call(_worker_class, job, _queue, _redis_pool)
          return yield unless Railspan.config.enabled?
          return yield unless Railspan::Context.current

          class_name = job["class"].to_s
          Railspan::Tracer.in_span(
            name: "job.enqueue",
            kind: "job.enqueue",
            resource: class_name,
            attributes: {
              "messaging.system" => "sidekiq",
              "messaging.destination" => job["queue"],
              "job.class" => class_name
            }.compact
          ) do
            yield
          end
        end
      end

      module_function

      def install!
        return if defined?(@installed) && @installed
        return unless defined?(::Sidekiq)

        @installed = true
        ::Sidekiq.configure_server do |config|
          config.server_middleware do |chain|
            chain.add(ServerMiddleware)
          end
        end
        ::Sidekiq.configure_client do |config|
          config.client_middleware do |chain|
            chain.add(ClientMiddleware)
          end
        end
      rescue StandardError => e
        warn "[railspan] Sidekiq install failed: #{e.message}" if ENV["RAILSPAN_DEBUG"]
      end
    end
  end
end
