# frozen_string_literal: true

require_relative "lib/railspan/version"

Gem::Specification.new do |spec|
  spec.name          = "railspan"
  spec.version       = Railspan::VERSION
  spec.authors       = ["Saidbek"]
  spec.email         = ["said.kaldybaev@gmail.com"]

  spec.summary       = "Lightweight Rails-first APM instrumentation"
  spec.description   = "Ruby SDK for Railspan: request traces, SQL spans, and export to the Railspan agent."
  spec.homepage      = "https://github.com/Saidbek/railspan"
  spec.license       = "MIT"
  spec.required_ruby_version = ">= 3.2.0"

  spec.metadata["homepage_uri"] = spec.homepage
  spec.metadata["source_code_uri"] = "https://github.com/Saidbek/railspan"

  spec.files = Dir.chdir(__dir__) do
    Dir["{lib}/**/*", "LICENSE.txt", "README.md"].select { |f| File.file?(f) }
  end
  spec.require_paths = ["lib"]

  spec.add_dependency "rack", ">= 2.0"

  spec.add_development_dependency "minitest", "~> 5.0"
  spec.add_development_dependency "rake", "~> 13.0"
end
