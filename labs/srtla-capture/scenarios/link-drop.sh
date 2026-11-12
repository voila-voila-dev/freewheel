# Link 2 silently cut for 10 s (sends succeed, nothing arrives: a dead zone), then 25 s of recovery.
LINKS=2
run() {
	start_all 45 1 2
	sleep 10
	cut_link 2
	sleep 10
	restore_link 2
	sleep 28
}
