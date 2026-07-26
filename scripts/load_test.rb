#!/usr/bin/env ruby
# frozen_string_literal: true

# Production-like load test against examples/dummy_rails + railspan serve.
#
# Simulates a mixed traffic profile (health, list, show, heavy N+1), concurrent
# clients, and optional flood phase. Snapshots /healthz before/after.
#
# Usage:
#   ruby scripts/load_test.rb
#   ruby scripts/load_test.rb --duration 60 --concurrency 20 --rps 100
#   ruby scripts/load_test.rb --flood --duration 30 --concurrency 50
#   BASE_URL=http://127.0.0.1:3000 RAILSPAN_URL=http://127.0.0.1:7421 ruby scripts/load_test.rb

require "net/http"
require "uri"
require "json"
require "optparse"
require "thread"

Options = Struct.new(
  :base_url,
  :railspan_url,
  :duration,
  :concurrency,
  :rps,
  :flood,
  :warmup,
  :api_key,
  keyword_init: true
)

def parse_options(argv)
  opts = Options.new(
    base_url: ENV.fetch("BASE_URL", "http://127.0.0.1:3000"),
    railspan_url: ENV.fetch("RAILSPAN_URL", "http://127.0.0.1:7421"),
    duration: Integer(ENV.fetch("DURATION", "45")),
    concurrency: Integer(ENV.fetch("CONCURRENCY", "12")),
    rps: ENV["RPS"] ? Float(ENV["RPS"]) : nil,
    flood: ENV["FLOOD"] == "1",
    warmup: Integer(ENV.fetch("WARMUP", "3")),
    api_key: ENV["RAILSPAN_API_KEY"] || ENV["RAILSPAN_UI_TOKEN"]
  )

  OptionParser.new do |o|
    o.banner = "Usage: ruby scripts/load_test.rb [options]"
    o.on("--base-url URL", "Rails app URL (default #{opts.base_url})") { |v| opts.base_url = v }
    o.on("--railspan-url URL", "Railspan server URL (default #{opts.railspan_url})") { |v| opts.railspan_url = v }
    o.on("-d", "--duration SEC", Integer, "Sustained phase seconds") { |v| opts.duration = v }
    o.on("-c", "--concurrency N", Integer, "Concurrent clients") { |v| opts.concurrency = v }
    o.on("--rps N", Float, "Target aggregate RPS (optional throttle)") { |v| opts.rps = v }
    o.on("--flood", "Aggressive flood mix (more N+1 + higher concurrency bias)") { opts.flood = true }
    o.on("--warmup SEC", Integer, "Warmup seconds (excluded from stats)") { |v| opts.warmup = v }
    o.on("--api-key KEY", "Bearer for railspan /api if required") { |v| opts.api_key = v }
  end.parse!(argv)

  opts
end

# Weighted production-like routes. Weights are relative.
def traffic_mix(flood:)
  if flood
    [
      { path: "/health", weight: 5, name: "health" },
      { path: "/users", weight: 30, name: "users.index" },
      { path: "/users/%{id}", weight: 25, name: "users.show", needs_id: true },
      { path: "/users/with_posts", weight: 40, name: "users.n1" }
    ]
  else
    [
      # LB / k8s probes — high volume, should be ignore_paths
      { path: "/health", weight: 20, name: "health" },
      { path: "/up", weight: 10, name: "up" },
      # Typical API traffic
      { path: "/users", weight: 35, name: "users.index" },
      { path: "/users/%{id}", weight: 25, name: "users.show", needs_id: true },
      # Occasional expensive page (N+1 demo)
      { path: "/users/with_posts", weight: 10, name: "users.n1" }
    ]
  end
end

class Histogram
  def initialize
    @samples = []
    @mutex = Mutex.new
  end

  def record(ms)
    @mutex.synchronize { @samples << ms }
  end

  def size
    @mutex.synchronize { @samples.size }
  end

  def snapshot
    @mutex.synchronize { @samples.dup }
  end

  def stats
    s = snapshot.sort
    return empty_stats if s.empty?

    {
      count: s.size,
      min: s.first,
      max: s.last,
      mean: (s.sum / s.size.to_f).round(2),
      p50: percentile(s, 50),
      p95: percentile(s, 95),
      p99: percentile(s, 99)
    }
  end

  def empty_stats
    { count: 0, min: 0, max: 0, mean: 0, p50: 0, p95: 0, p99: 0 }
  end

  def percentile(sorted, p)
    return 0 if sorted.empty?

    idx = ((p / 100.0) * (sorted.length - 1)).round
    sorted[idx].round(2)
  end
end

class LoadRunner
  def initialize(opts)
    @opts = opts
    @base = URI(opts.base_url)
    @mix = traffic_mix(flood: opts.flood)
    @total_weight = @mix.sum { |r| r[:weight] }
    @user_ids = (1..30).to_a # seed uses 10 users; show may 404 — still realistic
    @hist = Hash.new { |h, k| h[k] = Histogram.new }
    @global = Histogram.new
    @status = Hash.new(0)
    @errors = Hash.new(0)
    @status_mu = Mutex.new
    @stop = false
    @recording = false
  end

  def run!
    banner
    check_prereqs!
    before = railspan_health
    print_health("BEFORE", before)

    puts "\n== Warmup #{@opts.warmup}s =="
    run_phase(duration: @opts.warmup, record: false)

    puts "\n== Sustained load #{@opts.duration}s " \
         "(concurrency=#{@opts.concurrency}" \
         "#{@opts.rps ? ", target_rps=#{@opts.rps}" : ""}" \
         "#{@opts.flood ? ", FLOOD" : ""}) =="
    t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    run_phase(duration: @opts.duration, record: true)
    elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0

    # Let exporter flush
    sleep 1.5
    after = railspan_health
    print_report(elapsed, before, after)
    print_health("AFTER", after)
    print_delta(before, after)
    check_advice(after)
  end

  private

  def banner
    puts "=" * 60
    puts "Railspan production-like load test"
    puts "=" * 60
    puts "Rails:    #{@opts.base_url}"
    puts "Railspan: #{@opts.railspan_url}"
    puts "Mix:"
    @mix.each do |r|
      pct = (100.0 * r[:weight] / @total_weight).round(1)
      puts "  #{pct.to_s.rjust(5)}%  #{r[:name].ljust(14)} #{r[:path]}"
    end
  end

  def check_prereqs!
    code = http_get_code(URI.join(@opts.base_url + "/", "users"))
    abort "Rails not reachable at #{@opts.base_url} (got #{code}). Start with: just dummy" unless code == 200

    h = railspan_health
    abort "Railspan not reachable at #{@opts.railspan_url}/healthz" unless h && h["ok"]

    # Discover valid user ids
    body = http_get_body(URI.join(@opts.base_url + "/", "users"))
    if body
      ids = JSON.parse(body).map { |u| u["id"] }
      @user_ids = ids unless ids.empty?
    end
    puts "Discovered #{@user_ids.size} user ids for show traffic"
  rescue Errno::ECONNREFUSED => e
    abort "Connection refused: #{e.message}\nStart railspan serve + dummy_rails first."
  end

  def run_phase(duration:, record:)
    @recording = record
    @stop = false
    deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + duration
    interval = @opts.rps && @opts.rps > 0 ? (@opts.concurrency / @opts.rps.to_f) : 0

    threads = @opts.concurrency.times.map do
      Thread.new do
        # Per-thread persistent connection
        http = Net::HTTP.new(@base.host, @base.port)
        http.open_timeout = 2
        http.read_timeout = 10
        http.start
        begin
          while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline && !@stop
            route = pick_route
            path = expand_path(route)
            t0 = Process.clock_gettime(Process::CLOCK_MONOTONIC)
            begin
              req = Net::HTTP::Get.new(path)
              res = http.request(req)
              ms = (Process.clock_gettime(Process::CLOCK_MONOTONIC) - t0) * 1000.0
              if @recording
                @global.record(ms)
                @hist[route[:name]].record(ms)
                @status_mu.synchronize do
                  @status[res.code.to_i] += 1
                  @errors[:http] += 1 if res.code.to_i >= 500
                end
              end
            rescue StandardError => e
              if @recording
                @status_mu.synchronize { @errors[e.class.name] += 1 }
              end
            end
            sleep interval if interval.positive?
          end
        ensure
          http.finish rescue nil
        end
      end
    end

    # Progress
    start = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    while Process.clock_gettime(Process::CLOCK_MONOTONIC) < deadline
      sleep 1
      next unless record

      done = @global.size
      elapsed = Process.clock_gettime(Process::CLOCK_MONOTONIC) - start
      rps = elapsed.positive? ? (done / elapsed).round(1) : 0
      print "\r  requests=#{done}  ~rps=#{rps}  elapsed=#{elapsed.round(1)}s   "
      $stdout.flush
    end
    @stop = true
    threads.each(&:join)
    puts if record
  end

  def pick_route
    r = rand(@total_weight)
    @mix.each do |route|
      r -= route[:weight]
      return route if r.negative?
    end
    @mix.last
  end

  def expand_path(route)
    if route[:needs_id]
      format(route[:path], id: @user_ids.sample)
    else
      route[:path]
    end
  end

  def http_get_code(uri)
    Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 5) do |http|
      http.request(Net::HTTP::Get.new(uri)).code.to_i
    end
  end

  def http_get_body(uri)
    Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 5) do |http|
      res = http.request(Net::HTTP::Get.new(uri))
      res.body if res.is_a?(Net::HTTPSuccess)
    end
  end

  def railspan_health
    uri = URI.join(@opts.railspan_url.end_with?("/") ? @opts.railspan_url : "#{@opts.railspan_url}/", "healthz")
    Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 5) do |http|
      req = Net::HTTP::Get.new(uri)
      res = http.request(req)
      return nil unless res.is_a?(Net::HTTPSuccess)

      JSON.parse(res.body)
    end
  rescue StandardError
    nil
  end

  def railspan_api(path)
    uri = URI.join(@opts.railspan_url.end_with?("/") ? @opts.railspan_url : "#{@opts.railspan_url}/", path.sub(%r{\A/}, ""))
    Net::HTTP.start(uri.host, uri.port, open_timeout: 2, read_timeout: 10) do |http|
      req = Net::HTTP::Get.new(uri)
      req["Authorization"] = "Bearer #{@opts.api_key}" if @opts.api_key && !@opts.api_key.empty?
      res = http.request(req)
      return { status: res.code.to_i, body: nil } unless res.is_a?(Net::HTTPSuccess)

      { status: res.code.to_i, body: JSON.parse(res.body) }
    end
  rescue StandardError => e
    { status: 0, error: e.message }
  end

  def print_health(label, h)
    puts "\n-- Railspan /healthz (#{label}) --"
    if h.nil?
      puts "  (unavailable)"
      return
    end
    %w[
      ok spans_received spans_accepted spans_dropped_sample spans_dropped_cardinality
      batches_received batches_rejected traces_stored spans_stored n_plus_one_events
      advised_sample_rate
    ].each do |k|
      puts "  #{k}=#{h[k].inspect}" if h.key?(k)
    end
  end

  def print_delta(before, after)
    return unless before && after

    puts "\n-- Railspan delta --"
    %w[
      spans_received spans_accepted spans_dropped_sample spans_dropped_cardinality
      batches_received batches_rejected traces_stored spans_stored n_plus_one_events
    ].each do |k|
      b = before[k].to_i
      a = after[k].to_i
      puts "  Δ #{k}=#{a - b}"
    end
    if before["advised_sample_rate"] && after["advised_sample_rate"]
      puts "  advised_sample_rate #{before['advised_sample_rate']} → #{after['advised_sample_rate']}"
    end
  end

  def print_report(elapsed, _before, after)
    g = @global.stats
    total = g[:count]
    rps = elapsed.positive? ? (total / elapsed).round(2) : 0
    ok = @status.select { |c, _| c < 400 }.values.sum
    client_err = @status.select { |c, _| c >= 400 && c < 500 }.values.sum
    server_err = @status.select { |c, _| c >= 500 }.values.sum

    puts "\n" + "=" * 60
    puts "RESULTS"
    puts "=" * 60
    puts "Duration:     #{elapsed.round(2)}s"
    puts "Requests:     #{total}"
    puts "Throughput:   #{rps} req/s"
    puts "Success(<400):#{ok}"
    puts "4xx:          #{client_err}"
    puts "5xx:          #{server_err}"
    puts "Transport err:#{@errors.inspect}" unless @errors.empty?
    puts "Status codes: #{@status.sort.to_h}"
    puts
    puts "Latency (ms) — all:"
    print_lat(g)
    puts
    puts "Latency (ms) — by route:"
    @hist.keys.sort.each do |name|
      s = @hist[name].stats
      next if s[:count].zero?

      print "  #{name.ljust(14)} n=#{s[:count].to_s.rjust(6)}  "
      print_lat(s, indent: false)
    end

    # Optional endpoints summary from railspan API
    api = railspan_api("/api/v1/stats")
    if api[:status] == 200 && api[:body]
      puts "\n-- Railspan /api/v1/stats --"
      puts "  #{api[:body].inspect[0, 500]}"
    elsif api[:status] == 401
      puts "\n-- Railspan /api/v1/stats: 401 (set --api-key if auth enabled) --"
    end

    endpoints = railspan_api("/api/v1/endpoints?hours=1")
    if endpoints[:status] == 200 && endpoints[:body].is_a?(Array)
      puts "\n-- Top endpoints (last 1h) --"
      endpoints[:body].first(10).each do |ep|
        puts "  #{ep.inspect[0, 200]}"
      end
    end

    n1 = railspan_api("/api/v1/n-plus-one?hours=1")
    if n1[:status] == 200 && n1[:body].is_a?(Array) && !n1[:body].empty?
      puts "\n-- N+1 events detected --"
      n1[:body].first(5).each { |e| puts "  #{e.inspect[0, 200]}" }
    end

    puts "\nUI: #{@opts.railspan_url}/"
  end

  def print_lat(s, indent: true)
    prefix = indent ? "  " : ""
    fmt = ->(v) { v.is_a?(Numeric) ? v.round(2) : v }
    puts "#{prefix}min=#{fmt[s[:min]]}  p50=#{fmt[s[:p50]]}  p95=#{fmt[s[:p95]]}  " \
         "p99=#{fmt[s[:p99]]}  max=#{fmt[s[:max]]}  mean=#{fmt[s[:mean]]}"
  end

  def check_advice(after)
    return unless after

    rate = after["advised_sample_rate"]
    return unless rate.is_a?(Numeric)

    puts "\n-- Adaptive sampling --"
    if rate < 1.0
      puts "  advised_sample_rate=#{rate} (server under pressure — gem should adopt lower rate)"
    else
      puts "  advised_sample_rate=#{rate} (no pressure cap; try --flood or higher concurrency)"
    end
  end
end

LoadRunner.new(parse_options(ARGV)).run!
