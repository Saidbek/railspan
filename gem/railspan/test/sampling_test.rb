# frozen_string_literal: true

require_relative "test_helper"

class SamplingTest < Minitest::Test
  def test_sample_root_always_when_rate_one
    setup_railspan(sample_rate: 1.0)
    assert((1..20).all? { Railspan::Tracer.sample_root? })
  end

  def test_sample_root_never_when_rate_zero
    setup_railspan(sample_rate: 0.0)
    refute((1..20).any? { Railspan::Tracer.sample_root? })
  end

  def test_http_exporter_applies_advice
    setup_railspan(sample_rate: 1.0, exporter: :http)
    exporter = Railspan.exporter
    res = Object.new
    def res.is_a?(klass) = klass == Net::HTTPSuccess || super
    def res.body
      %({"ok":true,"accepted_spans":1,"dropped_spans":0,"advice":{"sample_rate":0.1}})
    end
    # Net::HTTPSuccess check — stub with a simple duck type
    success = Net::HTTPOK.new("1.1", "200", "OK")
    def success.body
      %({"ok":true,"accepted_spans":1,"dropped_spans":0,"advice":{"sample_rate":0.1}})
    end
    exporter.send(:apply_advice!, success)
    assert_in_delta 0.1, Railspan.config.sample_rate, 0.001
  end
end
