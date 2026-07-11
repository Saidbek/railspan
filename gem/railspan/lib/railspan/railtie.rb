# frozen_string_literal: true

module Railspan
  class Railtie < ::Rails::Railtie
    initializer "railspan.middleware", after: :load_config_initializers do |app|
      next unless Railspan.config.enabled?

      app.middleware.insert_before(0, Railspan::Middleware::Rack)
    end

    initializer "railspan.instrumentation" do
      ::ActiveSupport.on_load(:action_controller) do
        Railspan::Instrumentation::ActionController.install!
      end
      ::ActiveSupport.on_load(:active_record) do
        Railspan::Instrumentation::ActiveRecord.install!
      end
      ::ActiveSupport.on_load(:action_view) do
        Railspan::Instrumentation::ActionView.install!
      end

      # Install immediately if already loaded
      Railspan::Instrumentation::ActionController.install!
      Railspan::Instrumentation::ActiveRecord.install!
      Railspan::Instrumentation::ActionView.install!
    end

    config.after_initialize do
      Railspan.setup_exporter!
    end
  end
end
