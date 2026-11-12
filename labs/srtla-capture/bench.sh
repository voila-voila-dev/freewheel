#!/usr/bin/env bash
# Benchmarks senders and receivers on the same lab scenarios, a few runs of each.
#
#   labs/srtla-capture/bench.sh [runs=3] [parallel=1]
#
# Senders against upstream's receiver: upstream, freewheel, irlserver.
# Receivers behind upstream's sender: upstream, freewheel.
# Runs go to .cache/srtla-bench/<receiver>-<sender>/run-<n>/; then
#   cargo run --release -p packet-capture --bin bench-srtla -- .cache/srtla-bench
# prints the summary (best run for CPU, memory and delivery, median for recovery) and fails if ours is
# behind upstream. One scenario at a time by default: two at once disturb each other's CPU.
#
# Videos are not kept and captures keep headers only: a run is a few MB, not hundreds.
#
# Runs are interleaved (run 1 of every combination, then run 2…) so that a busier moment of the
# host does not land on one combination only.
set -euo pipefail

lab=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$lab/../.." && pwd)
runs=${1:-3}
parallel=${2:-1}
scenarios=(steady throughput aggregation link-drop)
pairs=(upstream/upstream upstream/freewheel upstream/irlserver freewheel/upstream)

rm -rf "$root/.cache/srtla-bench"
for run in $(seq 1 "$runs"); do
	for pair in "${pairs[@]}"; do
		receiver=${pair%/*}
		sender=${pair#*/}
		echo "run $run: $sender → $receiver"
		RECEIVER=$receiver SENDER=$sender DISCARD_VIDEO=1 CAPTURE_SNAPLEN=96 \
			CAPTURES_DIR="$root/.cache/srtla-bench/$receiver-$sender/run-$run" \
			"$lab/run-all.sh" "$parallel" "${scenarios[@]}"
	done
done
