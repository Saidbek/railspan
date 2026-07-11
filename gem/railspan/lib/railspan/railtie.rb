# frozen_string_literal: true

module Railspan
  class Railtie < ::Rails::Railtie
    initializer "railspan.middleware", after: :load_config_initializers do |app|
      next unless Railspan.config.enabled?

      app.middleware.insert_before(0, Railspan::Middleware::Rack)
    end

    initializer "railspan.instrumentation" do
      Railspan::Instrumentation::ActionController.install!
      Railspan::Instrumentation::ActiveRecord.install!
      Railspan::Instrumentation::ActionView.install!
      Railspan::Instrumentation::ActiveJob.install!
      Railspan::Instrumentation::Cache.install!
      Railspan::Instrumentation::NetHttp.install!
      Railspan::Instrumentation::Sidekiq.install!
    end

    config.after_initialize do
      Railspan.setup_exporter!
      Railspan::Instrumentation::Sidekiq.install!
    end
  end
end
