# Only the uplink of link 2 dies for 10 s; its downlink keeps delivering ACKs.
LINKS=2
run() {
	start_all 35 1 2
	sleep 10
	cut_link 2 up
	sleep 10
	restore_link 2 up
	sleep 18
}
