# 300 kbit/s stream: unflushed ACK batches and keepalives on idle links.
LINKS=2
BITRATE=300k
run() {
	start_all 30 1 2
	sleep 33
}
