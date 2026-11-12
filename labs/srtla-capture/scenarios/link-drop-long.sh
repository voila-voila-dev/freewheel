# Link 2 silently cut for 20 s: longer than the receiver's timeout, so it forgets the link.
LINKS=2
run() {
	start_all 45 1 2
	sleep 10
	cut_link 2
	sleep 20
	restore_link 2
	sleep 18
}
