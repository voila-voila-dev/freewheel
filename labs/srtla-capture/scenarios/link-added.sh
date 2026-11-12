# A third modem is plugged in mid-stream (SIGHUP): how fast does it take traffic?
LINKS=3
run() {
	start_all 40 1 2
	sleep 12
	reload_sender_links 1 1 2 3
	sleep 30
}
