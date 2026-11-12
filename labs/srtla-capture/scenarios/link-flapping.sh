# Link 2 goes down 1 s every 3 s, eight times: a modem on the edge of coverage.
LINKS=2
run() {
	start_all 35 1 2
	sleep 5
	for _ in $(seq 8); do
		cut_link 2
		sleep 1
		restore_link 2
		sleep 2
	done
	sleep 9
}
