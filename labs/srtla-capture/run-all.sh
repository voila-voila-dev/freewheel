#!/usr/bin/env bash
# Builds the lab and runs every scenario, a few at a time.
#
#   labs/srtla-capture/run-all.sh [parallel=4] [scenario…]                     upstream → .cache/srtla-captures/
#   RECEIVER=freewheel labs/srtla-capture/run-all.sh [parallel=4] [scenario…]  our receiver → .cache/srtla-captures-freewheel-receiver/
#   SENDER=freewheel labs/srtla-capture/run-all.sh [parallel=4] [scenario…]    our sender → .cache/srtla-captures-freewheel-sender/
#   both set to freewheel                                                     Rust ↔ Rust → .cache/srtla-captures-freewheel/
#   SENDER=irlserver: irlserver/srtla_send, from the `bench` image target   → .cache/srtla-captures-<receiver>-irlserver/
#   CAPTURES_DIR=<dir> overrides where the run goes (labs/srtla-capture/bench.sh uses it).
#   DISCARD_VIDEO=1 keeps only the frame digests and the received size; CAPTURE_SNAPLEN=<n> keeps
#   the first n bytes of each packet (scenarios may set their own).
set -euo pipefail

lab=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$lab/../.." && pwd)
parallel=${1:-4}
shift || true
receiver=${RECEIVER:-upstream}
sender=${SENDER:-upstream}
case "$receiver/$sender" in
	upstream/upstream) out="$root/.cache/srtla-captures" ;;
	freewheel/upstream) out="$root/.cache/srtla-captures-freewheel-receiver" ;;
	upstream/freewheel) out="$root/.cache/srtla-captures-freewheel-sender" ;;
	*/irlserver) out="$root/.cache/srtla-captures-$receiver-irlserver" ;;
	*) out="$root/.cache/srtla-captures-freewheel" ;;
esac
out=${CAPTURES_DIR:-$out}
target=lab
if [ "$sender" = irlserver ]; then target=bench; fi
# Created here, as the runner user: the build container runs as root and would own them.
mkdir -p "$out/runs" "$root/.cache/lab-target"
mounts=()
if [ "$receiver" = freewheel ] || [ "$sender" = freewheel ]; then
	# Built for Linux in the same Debian as the lab, so they run there unchanged.
	docker run --rm -v "$root:/src" -w /src -v freewheel-cargo-registry:/usr/local/cargo/registry \
		-e CARGO_TARGET_DIR=/src/.cache/lab-target rust:1.94.0-slim-trixie \
		cargo build --quiet --release --locked -p bonding-receiver -p bonding-sender
	mounts=(-v "$root/.cache/lab-target/release:/opt/freewheel:ro")
fi

docker build -q --target "$target" -t "freewheel-srtla-$target" "$lab" > /dev/null
scenarios=("$@")
if [ ${#scenarios[@]} -eq 0 ]; then
	scenarios=()
	for file in "$lab"/scenarios/*.sh; do scenarios+=("$(basename "$file" .sh)"); done
fi

run_one() {
	local scenario=$1
	if docker run --rm --privileged -e RECEIVER="$receiver" -e SENDER="$sender" -e DISCARD_VIDEO="${DISCARD_VIDEO:-0}" -e CAPTURE_SNAPLEN="${CAPTURE_SNAPLEN:-0}" ${mounts[@]+"${mounts[@]}"} -v "$out:/out" "freewheel-srtla-$target" "$scenario" > "$out/runs/$scenario.log" 2>&1; then
		tail -1 "$out/runs/$scenario.log"
	else
		echo "$scenario: FAILED (see $out/runs/$scenario.log)"
	fi
}

for scenario in "${scenarios[@]}"; do
	while [ "$(jobs -rp | wc -l)" -ge "$parallel" ]; do sleep 1; done
	run_one "$scenario" &
done
wait
