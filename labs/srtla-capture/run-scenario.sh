#!/usr/bin/env bash
# Runs one scenario (scenarios/<name>.sh) and writes, in /out:
#   <name>.box.pcap, <name>.server.pcap   SRTLA traffic seen by the box and by the server
#   <name>.marks                          what the scenario did, and when
#   <name>.ts, <name>.srt-stats.json      video received, SRT listener statistics
#   <name>.*.log                          every process
#
#   run-scenario <name>     one scenario
#   run-scenario --list     every scenario with its description
set -euo pipefail

readonly LAB=/opt/lab
export OUT=/out

# shellcheck source=/dev/null
for library in marks topology impairments processes capture inject; do source "$LAB/lib/$library.sh"; done

if [ "${1:-}" = --list ]; then
	for file in "$LAB"/scenarios/*.sh; do
		echo "$(basename "$file" .sh): $(sed -n '1s/^# //p' "$file")"
	done
	exit 0
fi

SCENARIO=${1:-}
if [ -z "$SCENARIO" ] || [ ! -f "$LAB/scenarios/$SCENARIO.sh" ]; then
	echo "usage: run-scenario <name> | --list" >&2
	exit 2
fi
export SCENARIO
mkdir -p "$OUT"
find "$OUT" -maxdepth 1 -name "$SCENARIO.*" -delete

# Each scenario sets LINKS (how many modems to create) and defines run().
# shellcheck source=/dev/null
source "$LAB/scenarios/$SCENARIO.sh"

create_namespaces
create_links "$LINKS"
start_captures
mark "scenario $SCENARIO"
run
stop_all
stop_captures
write_frame_digests
mark "done"
echo "$SCENARIO: $(stat -c %s "$OUT/$SCENARIO.ts" 2>/dev/null || echo 0) bytes of video received"
