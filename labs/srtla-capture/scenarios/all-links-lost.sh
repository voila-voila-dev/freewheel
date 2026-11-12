# Both links cut for 25 s: past the sender's 10 s give-up, the sender exits (upstream does).
LINKS=2
run() {
	start_all 50 1 2
	sleep 10
	cut_link 1
	cut_link 2
	sleep 25
	restore_link 1
	restore_link 2
	sleep 15
}
