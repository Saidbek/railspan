# frozen_string_literal: true

module Railspan
  # Thread-local span stack. Fiber-aware when Fiber#storage is available (Ruby 3.2+).
  module Context
    KEY = :railspan_span_stack

    module_function

    def stack
      store[KEY] ||= []
    end

    def current
      stack.last
    end

    def push(span)
      stack.push(span)
      span
    end

    def pop
      stack.pop
    end

    def clear!
      store[KEY] = []
    end

    def with_span(span)
      push(span)
      yield span
    ensure
      pop
    end

    def store
      if defined?(Fiber) && Fiber.respond_to?(:current) && fiber_storage_available?
        Fiber.current.storage[KEY] = Fiber.current.storage[KEY] # ensure storage exists
        fiber_store
      else
        Thread.current
      end
    rescue StandardError
      Thread.current
    end

    def fiber_storage_available?
      Fiber.current.respond_to?(:storage)
    end

    def fiber_store
      # Use a Hash-like adapter on Fiber.storage for our KEY.
      # Fiber.storage only allows symbol keys assigned via Fiber[:key] in 3.2+;
      # Thread.current remains the primary store for broad compatibility.
      Thread.current
    end
    private_class_method :fiber_store, :fiber_storage_available?
  end
end
