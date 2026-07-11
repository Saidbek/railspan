# frozen_string_literal: true

$LOAD_PATH.unshift File.expand_path("../lib", __dir__)
require "railspan"
require "minitest/autorun"
require "json"

module Railspan
  module TestHelpers
    def setup_railspan(exporter: :null, **opts)
      Railspan.reset!
      Railspan.configure do |c|
        c.exporter = exporter
        c.enabled = true
        opts.each { |k, v| c.public_send("#{k}=", v) }
      end
      if exporter == :null
        Railspan.exporter = MemoryExporter.new
      end
    end

    class MemoryExporter
      attr_reader :spans

      def initialize
        @spans = []
        @mutex = Mutex.new
      end

      def export(span)
        @mutex.synchronize { @spans << span.to_h }
      end

      def shutdown; end
    end
  end
end

Minitest::Test.include Railspan::TestHelpers
