#!/usr/bin/env gnuplot

data_file="/tmp/data.log"  # data preprocessed from plot.awk

# Other configuration
graph_width=2000
graph_height=600

moving_avg_N = 3           # for Kilowatt. 1 => no averaging
timezone_diff_to_GMT = +1  # To correctly print the timestamp

# Figure out input range, needed for day cutoff background boxes
stats data_file using 1 nooutput

set terminal kittycairo scroll size graph_width,graph_height

#set terminal png size graph_width,graph_height
#set output "/tmp/graph.png"

set key at graph 0.01, 0.85 left top

#-- Background boxes and weekday labels
set style rectangle back fc rgb "#fafafa" fs solid 1.0 noborder

# Ranges for the background loop
t_start = STATS_min
t_end   = STATS_max
one_day = 24 * 3600

t_start = t_start - (int(t_start) % one_day)

do for [t = t_start : t_end : one_day] {
    day_name = strftime("%A", t)
    date_txt = strftime("%Y-%m-%d", t)

    t_mid = t + (one_day / 2)
    # Place the label at the top (graph 0.95)
    set label day_name at t, graph 0.95 left font "Sans-Serif,16"
    set label date_txt at t, graph 0.90 left font "Sans-Serif,12"
    day_index = int((t - t_start) / one_day)
    if (day_index % 2 == 0) {
      # Color background boxes
      set object rect from t, graph 0 to t+one_day, graph 1 behind
    }
}

set ylabel "m³"
set y2label "kW"
set y2tics ; set ytics nomirror
set grid y2tics

set xdata time
set timefmt "%s"
set format x "%H:%M"
set xtics rotate by 45 nomirror right
set xtics 7200

set format y "%.1f"
set format y2 "%.1f"

array A[moving_avg_N]
samples(x) = $0 > (moving_avg_N - 1) ? moving_avg_N : int($0+1)
mod(x) = int(x) % moving_avg_N
avg_n(x) = (A[mod($0)+1]=x, (sum [i=1:samples($0)] A[i]) / samples($0))

tz_adjust(x) = x + timezone_diff_to_GMT * 3600

plot data_file using (tz_adjust($1)):2 title "Gas-Meter in m³", \
     ""        using (tz_adjust($1)):(avg_n($3)) axes x1y2 with lines lw 2 title "Power use in Kilowatt (avg)"
