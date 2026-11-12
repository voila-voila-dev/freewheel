# Only the downlink of link 2 dies for 10 s: its data arrives, but the box hears nothing back.
LINKS=2
run() {
	start_all 35 1 2
	sleep 10
	cut_link 2 down
	sleep 10
	restore_link 2 down
	sleep 18
}
