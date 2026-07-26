# frozen_string_literal: true

module Railspan
  # Walks the Ruby call stack and picks the first *application* frame.
  # Used to attribute SQL/cache/HTTP spans to app file:line (see docs/SOURCE_LOCATIONS.md).
  module CallerLocation
    module_function

    MAX_DEPTH = 48
    MAX_PATH = 512
    MAX_FUNC = 128

    # @return [Hash, nil] attributes keyed code.filepath / code.lineno / code.function
    def capture(skip: 1)
      return nil unless Railspan.config.capture_source_location

      locs = caller_locations(skip, MAX_DEPTH)
      return nil if locs.nil? || locs.empty?

      frame = locs.find { |loc| app_frame?(loc) }
      return nil unless frame

      path = frame.absolute_path || frame.path
      return nil if path.nil? || path.empty?

      rel = relativize(path)
      func = frame.base_label.to_s
      func = func[0, MAX_FUNC] if func.length > MAX_FUNC

      attrs = {
        "code.filepath" => rel[0, MAX_PATH],
        "code.lineno" => frame.lineno
      }
      attrs["code.function"] = func unless func.empty?
      attrs
    rescue StandardError
      nil
    end

    def app_frame?(loc)
      path = loc.absolute_path || loc.path
      return false if path.nil? || path.empty?
      return false if library_path?(path)

      root = app_root
      if root
        return false unless path.start_with?(root)
        # exclude vendored code inside the app tree
        return false if path.include?("#{root}/vendor/")
        return false if path.include?("#{root}/node_modules/")

        return true
      end

      # Non-Rails / tests: accept common app layouts
      path.match?(%r{/(app|lib|config|spec|test)/}) && !path.include?("/gems/")
    end

    def library_path?(path)
      path.include?("/gems/") ||
        path.include?("/ruby/") ||
        path.include?("vendor/bundle") ||
        path.include?("lib/railspan/") ||
        path.include?("/bundler/") ||
        path.end_with?("railspan/caller_location.rb") ||
        path.include?("/railties-") ||
        path.include?("/activesupport-") ||
        path.include?("/activerecord-") ||
        path.include?("/actionpack-") ||
        path.include?("/actionview-") ||
        path.include?("/activejob-") ||
        path.include?("/sidekiq-")
    end

    def app_root
      if defined?(::Rails) && Rails.respond_to?(:root) && Rails.root
        return Rails.root.to_s
      end

      Railspan.config.application_root
    end

    def relativize(path)
      root = app_root
      if root && path.start_with?(root)
        rel = path.sub(%r{\A#{Regexp.escape(root)}/?}, "")
        return rel.empty? ? path : rel
      end
      path
    end
  end
end
