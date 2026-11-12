# Link 2 jumps from 20 ms to 1.5 s one-way for 5 s (a handover or a congested cell), then back.
LINKS=2
run() {
	start_all 30 1 2
	sleep 10
	shape_link 2 both "delay 1500ms"
	mark "latency spike on link 2"
	sleep 5
	restore_link 2
	sleep 18
}
