#!/usr/bin/env bash
# Benchmarks transports on the scenarios M1b is judged on, a few runs of each.
#
#   labs/srtla-capture/bench-transport.sh [runs=3] [parallel=1] [receiver/sender…]
#
# Default combinations: upstream/upstream (the baseline: SRTLA + SRT), freewheel/freewheel (our
# compatibility mode) and upstream/irlserver. Each candidate of M1b joins as another pair.
# Runs go to .cache/transport-bench/<receiver>-<sender>/run-<n>/; then
#   cargo run --release -p packet-capture --bin bench-transport -- .cache/transport-bench
# prints frames lost, longest gap, delivered bitrate and latency per scenario
# (`--gate <receiver>-<sender>` checks a candidate against the baseline).
#
# Scenarios: a dead link, bursty loss, varying capacity, a total outage, a ride with everything
# at once, and two replays of recorded LTE traces (in town and on the road).
# Runs are interleaved (run 1 of every combination, then run 2…) so that a busier moment of the
# host does not land on one combination only.
set -euo pipefail

lab=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$lab/../.." && pwd)
runs=${1:-3}
parallel=${2:-1}
shift 2 || shift $#
scenarios=(link-drop burst-loss varying-bandwidth all-links-drop real-ride trace-city trace-drive)
pairs=("$@")
if [ ${#pairs[@]} -eq 0 ]; then pairs=(upstream/upstream freewheel/freewheel upstream/irlserver); fi

rm -rf "$root/.cache/transport-bench"
for run in $(seq 1 "$runs"); do
	for pair in "${pairs[@]}"; do
		receiver=${pair%/*}
		sender=${pair#*/}
		echo "run $run: $sender → $receiver"
		RECEIVER=$receiver SENDER=$sender DISCARD_VIDEO=1 CAPTURE_SNAPLEN=96 \
			CAPTURES_DIR="$root/.cache/transport-bench/$receiver-$sender/run-$run" \
			"$lab/run-all.sh" "$parallel" "${scenarios[@]}"
	done
done
