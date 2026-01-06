#!/usr/bin/env -S awk -f
BEGIN {
    last_time=0;
    last_gas=0;
    KILOWATT_HOUR_PER_CUBIC_METER=10.57131
    IMPLAUSIBLE_KILOWATT = 25
    DATE_FORMAT = "%Y-%m-%d %H:%M"
}

{
    if (last_time > 0) {
        delta_t_h = ($1 - last_time) / 3600.0;
        delta_gas_m3 = ($2 - last_gas) / 100.0;
        m3_per_hour=delta_gas_m3 / delta_t_h;
        kw=m3_per_hour * KILOWATT_HOUR_PER_CUBIC_METER;

        # Some plasibility check
        if (delta_gas_m3 < 0) {
            print strftime(DATE_FORMAT, $1) ": Backwards value " $2 " last was " last_gas > "/dev/stderr"
            next;
        }
        if (kw > IMPLAUSIBLE_KILOWATT) {
            print strftime(DATE_FORMAT, $1) ": Jump value " $2 " last was " last_gas > "/dev/stderr"
            next;
        }

        # Raw gas use in cubic meter and derivation converted Kilowatt
        printf("%d %.2f %f\n", $1, ($2 / 100.0), kw);
    }
    last_time=$1;
    last_gas=$2;
}
