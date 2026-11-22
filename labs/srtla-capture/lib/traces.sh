# Replays of recorded cellular traces on a link's uplink.
#
# Traces are Mahimahi's (one line per 1500-byte delivery opportunity, in ms), downloaded by
# run-all.sh into .cache/traces and mounted at /opt/traces. They are GPL-3.0: we only read them.

readonly TRACES=/opt/traces
# One Mahimahi line is one 1500-byte opportunity: 12 kbit.
readonly TRACE_KBIT_PER_OPPORTUNITY=12
# What a trace does not say: the one-way delay and the modem's queue, in packets.
readonly TRACE_ONE_WAY_DELAY=40ms
readonly TRACE_QUEUE_PACKETS=100
readonly NANOSECONDS_PER_SECOND=1000000000

# replay_trace <k> <trace> <first second> <seconds>: link k's uplink capacity follows the trace,
# one step per second, from <first second> on, in the background. A second without any
# opportunity is a dead second (everything sent is lost).
replay_trace() {
	local k=$1 trace=$2 first=$3 seconds=$4
	local file="$TRACES/$trace"
	if [ ! -s "$file" ]; then
		echo "missing trace $file (run-all.sh downloads it)" >&2
		exit 2
	fi
	local rates
	rates=$(awk -v first="$first" -v seconds="$seconds" -v kbit="$TRACE_KBIT_PER_OPPORTUNITY" '
		{ second = int($1 / 1000) - first; if (second >= 0 && second < seconds) count[second]++ }
		END { for (i = 0; i < seconds; i++) print (count[i] + 0) * kbit }' "$file")
	(
		local start step=0 rate remaining_ms
		start=$(date +%s%N)
		for rate in $rates; do
			if [ "$rate" -eq 0 ]; then
				shape_link "$k" up "loss 100%"
			else
				shape_link "$k" up "delay $TRACE_ONE_WAY_DELAY rate ${rate}kbit limit $TRACE_QUEUE_PACKETS"
			fi
			step=$((step + 1))
			remaining_ms=$(((start + step * NANOSECONDS_PER_SECOND - $(date +%s%N)) / 1000000))
			if [ "$remaining_ms" -gt 0 ]; then sleep "$((remaining_ms / 1000)).$(printf %03d $((remaining_ms % 1000)))"; fi
		done
	) &
	mark "link $k replays $trace from ${first}s for ${seconds}s"
}
