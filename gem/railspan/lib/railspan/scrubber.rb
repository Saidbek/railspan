# frozen_string_literal: true

module Railspan
  module Scrubber
    REDACTED = "[REDACTED]"

    module_function

    def scrub_attributes(attrs, keys:)
      return {} if attrs.nil?

      denylist = keys.map { |k| k.to_s.downcase }
      attrs.each_with_object({}) do |(key, value), out|
        k = key.to_s
        out[k] = if sensitive_key?(k, denylist)
                   REDACTED
                 else
                   scrub_value(value, denylist)
                 end
      end
    end

    def sensitive_key?(key, denylist)
      lower = key.downcase
      denylist.any? { |d| lower == d || lower.end_with?(".#{d}") || lower.include?(d) }
    end

    def scrub_value(value, denylist)
      case value
      when Hash
        scrub_attributes(value, keys: denylist)
      when Array
        value.map { |v| scrub_value(v, denylist) }
      else
        value
      end
    end
  end
end
