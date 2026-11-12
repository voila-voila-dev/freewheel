# The receiver starts 6 s after the sender: registration retries, video lost meanwhile.
LINKS=2
run() {
	start_srt_listener
	start_sender 1 1 2
	start_stream 1 30 now
	sleep 6
	start_receiver
	sleep 27
}
