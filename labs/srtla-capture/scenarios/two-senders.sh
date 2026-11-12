# Two boxes on the same receiver, two links each: two groups, kept apart.
LINKS=4
run() {
	start_srt_listener
	start_receiver
	start_sender 1 1 2
	start_sender 2 3 4
	start_stream 1 30
	start_stream 2 30
	sleep 33
}
