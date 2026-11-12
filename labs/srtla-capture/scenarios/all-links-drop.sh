# Both links cut for 6 s (a tunnel), under the sender's 10 s give-up: does it all come back?
LINKS=2
run() {
	start_all 35 1 2
	sleep 10
	cut_link 1
	cut_link 2
	sleep 6
	restore_link 1
	restore_link 2
	sleep 22
}
