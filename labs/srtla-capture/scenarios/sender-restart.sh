# The sender restarts mid-stream (new group id, new ports); the old group lingers on the server.
LINKS=2
run() {
	start_all 40 1 2
	sleep 12
	stop_sender 1
	sleep 1
	start_sender 1 1 2
	sleep 30
}
