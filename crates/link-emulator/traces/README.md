# Cellular traces

Mahimahi format: one line per delivery opportunity (millisecond timestamp, one 1500-byte packet each), looping.

## Public traces to replay (check each licence before vendoring; otherwise fetch at test time)

- Mahimahi LTE traces (Verizon, T-Mobile, AT&T, up- and downlink), `ravinet/mahimahi` repository, `traces/`.
- Pantheon calibrated cellular traces (Stanford).

## Our own traces (M3)

Recorded on the bike with the box: per-link capacity, delay, loss, signal (RSRP/SINR), cell changes, GPS.
They become the public benchmark suite.
