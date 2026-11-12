# Total capacity (2 × 1.5 Mbit/s) below the 4 Mbit/s stream: NAKs, queues, late packets.
LINKS=2
BITRATE=4M
run() {
	shape_link 1 up "delay 20ms rate 1500kbit limit 100"
	shape_link 2 up "delay 20ms rate 1500kbit limit 100"
	start_all 30 1 2
	sleep 33
}
