#!/usr/bin/env -S gnuplot

DATA_FILE="/tmp/data.log"
MOVING_AVG_POINTS = 3   # for kWh. 1 => no averaging

set terminal kittycairo scroll size 800,600

#set terminal png
#set output "/tmp/graph.png"

set key left top
set ylabel "m³"
set y2label "kWh"
set y2tics ; set ytics nomirror

set xdata time
set timefmt "%s"
set format x "(%Y-%m-%d) %H:%M"
set xtics rotate by 45 nomirror right


array A[MOVING_AVG_POINTS]
samples(x) = $0 > (MOVING_AVG_POINTS - 1) ? MOVING_AVG_POINTS : int($0+1)
mod(x) = int(x) % MOVING_AVG_POINTS
avg_n(x) = (A[mod($0)+1]=x, (sum [i=1:samples($0)] A[i]) / samples($0))

plot DATA_FILE using ($1+(1*3600)):2 with lines title "Gas-Meter", \
     ""              using ($1+(1*3600)):(avg_n($3)) axes x1y2 with lines title "kWh"
