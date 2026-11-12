# The SRT listener behind the receiver restarts: srtla_rec loses its upstream socket.
LINKS=2
run() {
	start_all 40 1 2
	sleep 12
	stop_srt_listener
	sleep 2
	start_srt_listener
	mark "SRT listener started"
	sleep 28
}
