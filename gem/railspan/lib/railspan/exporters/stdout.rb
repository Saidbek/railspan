# frozen_string_literal: true

require "json"

module Railspan
  module Exporters
    class Stdout
      def export(span)
        $stdout.puts(JSON.generate(span.to_h))
        $stdout.flush
      rescue StandardError => e
        warn "[railspan] stdout export failed: #{e.class}: #{e.message}"
      end

      def shutdown; end
    end
  end
end
