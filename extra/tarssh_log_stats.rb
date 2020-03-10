#!/usr/bin/env ruby

require 'time'

def ft(t)
  if t >= 86400
    "%.1fd" % (t / 86400.0)
  elsif t >= 3600
    "%.1fh" % (t / 3600.0)
  elsif t >= 60
    "%.1fm" % (t / 60.0)
  else
    "%.0fs" % t
  end
end

history = []
clients = {}
startup = nil

ARGF.each_line do |line|
  prefix, data = line.split('] ', 2)
  ts = Time.iso8601(prefix[/\d+\S+/])
  case data
  when /^start,/
    clients.clear
    history.clear
    startup = ts
  when /^disconnect,/
    if line =~ /peer: ([^,]+)/
      if (connect_ts = clients.delete($1))
        history << (ts - connect_ts).to_f
      else
        warn "Can't find #{$1}"
      end
    else
      warn "Can't parse #{line}"
    end
  when /^connect,/
    if line =~ /peer: ([^,]+)/
      clients[$1] = ts
    else
      warn "Can't parse #{line}"
    end
  end
end

now = Time.new
history += clients.values.map {|v| (now - v).to_f }
history.sort!

exit if history.empty?

min = history.first
max = history.last
med = history[history.length / 2]
avg = history.sum.to_f / history.length
stddev = Math.sqrt(history.sum { |i| (i - avg) ** 2 } / (history.length - 1).to_f)

puts "Uptime: %s" % ft((now - startup))
puts "Current Clients: #{clients.size} of #{history.size} total"
puts("Connect Times: %s - %s, median %s, avg %s, stddev %s" % [ft(min), ft(max), ft(med), ft(avg), ft(stddev)])

log_history = history.group_by {|time| Math.log2(time).ceil }

puts "Breakdown:"
log_history.keys.sort.each do |k|
  a, b = 2 ** (k - 1), 2 ** k
  puts("%16s: %d" % ["#{ft(a)} - #{ft(b)}", log_history[k].size])
end

puts
puts "Current clients:"
max = clients.keys.map(&:length).max
ts = clients.values.map(&:to_s).map(&:length).max

puts("%#{max}s | %#{ts}s | %s" % ["Peer", "Timestamp", "Connection time"])
clients.each do |client, timestamp|
  puts("%#{max}s | %s | %s" % [client, timestamp, ft(now - timestamp)])
end

