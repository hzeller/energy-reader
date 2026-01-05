#!/usr/bin/env gnuplot

data_file="/tmp/data.log"  # data preprocessed from plot.awk

# Other configuration
graph_width=2000
graph_height=600

moving_avg_N = 6           # for kWh. 1 => no averaging
timezone_diff_to_GMT = +1  # To correctly print the timestamp

set terminal kittycairo scroll size graph_width,graph_height

#set terminal png size graph_width,graph_height
#set output "/tmp/graph.png"

set key left top
set ylabel "m³"
set y2label "kWh"
set y2tics ; set ytics nomirror
set grid y2tics

set xdata time
set timefmt "%s"
set format x "(%Y-%m-%d) %H:%M"
set xtics rotate by 45 nomirror right

set format y "%.1f"
set format y2 "%.1f"

array A[moving_avg_N]
samples(x) = $0 > (moving_avg_N - 1) ? moving_avg_N : int($0+1)
mod(x) = int(x) % moving_avg_N
avg_n(x) = (A[mod($0)+1]=x, (sum [i=1:samples($0)] A[i]) / samples($0))

tz_adjust(x) = x + timezone_diff_to_GMT * 3600

plot data_file using (tz_adjust($1)):2 title "Gas-Meter", \
     ""        using (tz_adjust($1)):(avg_n($3)) axes x1y2 with lines lw 2 title "kWh (avg)"
