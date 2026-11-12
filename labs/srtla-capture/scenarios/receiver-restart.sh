# The server's srtla_rec restarts mid-stream and forgets every group.
LINKS=2
run() {
	start_all 40 1 2
	sleep 12
	stop_receiver
	sleep 1
	start_receiver
	sleep 30
}
