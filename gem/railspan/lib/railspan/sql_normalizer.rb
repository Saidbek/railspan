# frozen_string_literal: true

module Railspan
  module SqlNormalizer
    MAX_LENGTH = 2_000

    module_function

    def normalize(sql)
      return "" if sql.nil?

      s = sql.to_s.dup
      # Strip Rails SQL comment annotations: /*...*/
      s = s.gsub(%r{/\*.*?\*/}m, "")
      # Single-quoted string literals only (double quotes = identifiers)
      s = s.gsub(/'(?:[^'\\]|\\.|'')*'/, "?")
      # Numbers (avoid matching identifiers with digits mid-token poorly — word boundaries OK)
      s = s.gsub(/\b\d+\.\d+\b/, "?")
      s = s.gsub(/\b\d+\b/, "?")
      s = s.gsub(/\s+/, " ").strip
      # IN (?, ?, ?) -> IN (?)
      s = s.gsub(/IN\s*\(\s*\?(?:\s*,\s*\?)*\s*\)/i, "IN (?)")
      s = s[0, MAX_LENGTH] if s.length > MAX_LENGTH
      s
    end
  end
end
