#!/usr/bin/env ruby
# frozen_string_literal: true

# Micro-benchmark: span create/finish cost with railspan gem on vs null exporter.
# Usage: ruby scripts/bench_overhead.rb [iterations]
# Budget: gem path should stay well under a few microseconds per span on modern hardware;
# full request overhead is documented in docs/OVERHEAD.md (target < ~2% vs baseline).

require "benchmark"
root = File.expand_path("..", __dir__)
$LOAD_PATH.unshift File.join(root, "gem/railspan/lib")
require "railspan"

ITER = (ARGV[0] || "50_000").to_s.delete("_").to_i

def run_loop(n)
  n.times do
    Railspan::Tracer.in_span(name: "bench", kind: "custom", resource: "Bench#run") do |span|
      span.attributes["i"] = 1
    end
  end
end

def measure(label)
  # warmup
  run_loop(1_000)
  t = Benchmark.realtime { run_loop(ITER) }
  per = (t / ITER) * 1_000_000.0
  puts format("%-18s  total=%.4fs  per_span=%.2f µs  thruput=%.0f spans/s", label, t, per, ITER / t)
  [t, per]
end

Railspan.reset!
Railspan.configure do |c|
  c.enabled = false
  c.exporter = :null
end
_off_t, off_us = measure("gem disabled")

Railspan.reset!
Railspan.configure do |c|
  c.enabled = true
  c.exporter = :null
  c.sample_rate = 1.0
end
# Memory exporter still does work via NullSpan path when... actually exporter null means
# Railspan.exporter is nil, finish still scrubs and calls exporter&.export
_on_t, on_us = measure("gem enabled null")

delta = on_us - off_us
puts
puts format("delta per span: %.2f µs", delta)
puts "Budget (microbench): prefer < 25 µs/span local; see docs/OVERHEAD.md for request-level <2%."
# Soft fail only if catastrophically slow (CI signal)
if on_us > 500
  warn "FAIL: enabled path > 500 µs/span (#{on_us.round(1)} µs) — investigate hot path"
  exit 1
end
puts "OK"
