# Ten minutes on two links: does anything drift, leak or stall? Headers-only captures.
LINKS=2
CAPTURE_SNAPLEN=320
readonly LONG_RUN_MINUTES=10
run() {
	start_all $((LONG_RUN_MINUTES * 60)) 1 2
	sleep 30
	for _ in $(seq 1 "$LONG_RUN_MINUTES"); do
		sample_memory
		sleep 57
	done
	sleep 3
}
