# The recording upload (M1b step 6, README "Recording upload"): while stream <s> runs, the box
# records random bytes at $RECORDING_BPS into $RECORDING_PATH_PREFIX.<s>.bin, freewheel-sender
# uploads them, and freewheel-receiver writes its copy in $RECORDING_COPY_DIR. Once the recording
# is written, a watcher compares the copy with it every $RECORDING_POLL_S until it is whole or the
# drain time is over; then <scenario>.recording says how much arrived (recording-check).
#
# Marks: "recording <s> started …", "recording <s> written <bytes> bytes" (the stream's end),
# then "recording <s> delivered" or "recording <s> drain over".

readonly RECORDING_PATH_PREFIX=/tmp/freewheel-recording
readonly RECORDING_COPY_DIR=/tmp/freewheel-recordings
readonly RECORDING_POLL_S=0.5
# Compares the copy with the recording (crates/packet-capture), mounted by run-all.sh.
readonly RECORDING_CHECK=/opt/freewheel/recording-check
readonly BITS_PER_BYTE=8
RECORDING_WATCHERS=()

# recording_on: the run records (RECORDING=1, and our transport on both ends).
recording_on() {
	[ "$RECORDING" = 1 ] && [ "${SENDER:-upstream}" = transport ] && [ "${RECEIVER:-upstream}" = transport ]
}

# recording_path <s>: the recording of stream s.
recording_path() {
	echo "$RECORDING_PATH_PREFIX.$1.bin"
}

# start_recording <s> <seconds>: stream s's recording, written for <seconds>, then watched for
# the drain time ($RECORDING_DRAIN_S, or <seconds>).
start_recording() {
	local sender=$1 seconds=$2
	local path drain_s
	path=$(recording_path "$sender")
	drain_s=${RECORDING_DRAIN_S:-$seconds}
	: > "$path"
	(
		head -c "$((RECORDING_BPS * seconds / BITS_PER_BYTE))" /dev/urandom |
			pv -q -L "$((RECORDING_BPS / BITS_PER_BYTE))" >> "$path"
		mark "recording $sender written $(stat -c %s "$path") bytes"
		watch_recording_copy "$sender" "$drain_s"
	) &
	RECORDING_WATCHERS+=($!)
	mark "recording $sender started at $RECORDING_BPS bit/s for ${seconds}s, drained up to ${drain_s}s"
}

# recording_copy: the server's copy (the largest, if a sender restarted into a new session).
recording_copy() {
	# shellcheck disable=SC2012 # names are hex digits
	ls -S "$RECORDING_COPY_DIR"/*.recording 2> /dev/null | head -1
}

# watch_recording_copy <s> <drain seconds>: marks when the copy is whole, or that it was not
# after the drain time.
watch_recording_copy() {
	local sender=$1 drain_s=$2
	local deadline copy
	deadline=$(($(date +%s) + drain_s))
	while [ "$(date +%s)" -lt "$deadline" ]; do
		copy=$(recording_copy)
		if [ -n "$copy" ] && cmp -s "$(recording_path "$sender")" "$copy"; then
			mark "recording $sender delivered"
			return
		fi
		sleep "$RECORDING_POLL_S"
	done
	mark "recording $sender drain over"
}

# finish_recording: waits for the watchers (the links stay up meanwhile), then writes
# <scenario>.recording for stream 1 and removes both files.
finish_recording() {
	recording_on || return 0
	local watcher
	for watcher in "${RECORDING_WATCHERS[@]}"; do wait "$watcher" || true; done
	if [ -f "$(recording_path 1)" ]; then
		"$RECORDING_CHECK" "$(recording_path 1)" "$(recording_copy)" > "$OUT/$SCENARIO.recording" || true
		mark "recording 1 compared: $(cat "$OUT/$SCENARIO.recording")"
	fi
	rm -rf "$RECORDING_PATH_PREFIX".*.bin "$RECORDING_COPY_DIR"
}
