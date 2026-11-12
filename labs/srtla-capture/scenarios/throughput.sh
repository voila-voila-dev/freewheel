# 50 Mbit/s of SRT through two clean links, 20 s: what forwarding costs in CPU and memory.
LINKS=2
CAPTURE_SNAPLEN=64
run() {
	start_srt_listener
	start_receiver
	start_sender 1 1 2
	start_raw_stream 1 20 50000000
	sleep 23
}
