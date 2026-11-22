# Timestamped marks, read by analyze-srtla-capture to place events on the timeline.
# One line per mark: <microseconds since the Unix epoch> <label>.

mark() {
	echo "$(date +%s%6N) $*" >> "$OUT/$SCENARIO.marks"
	echo "[$(date +%T)] $*"
}

# quiet_mark <label>: a mark that is not echoed (one per second of a trace replay).
quiet_mark() {
	echo "$(date +%s%6N) $*" >> "$OUT/$SCENARIO.marks"
}
