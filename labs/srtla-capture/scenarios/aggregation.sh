# 12 Mbit/s of SRT through two 8 Mbit/s links, 20 s: only bonding both carries it all.
LINKS=2
CAPTURE_SNAPLEN=64
run() {
	shape_link 1 up "delay 20ms rate 8mbit limit 200"
	shape_link 2 up "delay 20ms rate 8mbit limit 200"
	start_srt_listener
	start_receiver
	start_sender 1 1 2
	start_raw_stream 1 20 12000000
	sleep 23
}
