# railspan (Ruby gem)

Instrumentation SDK for [Railspan](https://github.com/Saidbek/railspan).

## Install

```ruby
# Gemfile
gem "railspan", path: "../../gem/railspan" # monorepo
```

## Configure

```ruby
Railspan.configure do |c|
  c.service_name = "my-app"
  c.environment  = "development"
  c.endpoint     = "http://127.0.0.1:7421"
  c.api_key      = ENV["RAILSPAN_API_KEY"]
  c.exporter     = :http # or :stdout
  c.enabled      = true
end
```

## ENV overrides

| Variable | Maps to |
|----------|---------|
| `RAILSPAN_ENABLED` | `enabled` (`true`/`false`) |
| `RAILSPAN_SERVICE_NAME` | `service_name` |
| `RAILSPAN_ENVIRONMENT` | `environment` |
| `RAILSPAN_ENDPOINT` | `endpoint` |
| `RAILSPAN_API_KEY` | `api_key` |
| `RAILSPAN_EXPORTER` | `exporter` (`http`/`stdout`) |
| `RAILSPAN_SAMPLE_RATE` | `sample_rate` |
