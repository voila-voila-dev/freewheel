# Modem 2 is removed from the list mid-stream (SIGHUP): clean hand-over to link 1?
LINKS=2
run() {
	start_all 35 1 2
	sleep 12
	reload_sender_links 1 1
	sleep 25
}
