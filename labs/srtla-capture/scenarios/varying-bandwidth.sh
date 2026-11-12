# Link 2's capacity changes every 8 s, as a moving bike crosses cells: 8, 1, 4, 0.5, then 8 Mbit/s.
LINKS=2
run() {
	shape_link 1 up "delay 20ms rate 4mbit limit 50"
	start_all 55 1 2
	sleep 5
	for rate in 8mbit 1mbit 4mbit 500kbit 8mbit; do
		shape_link 2 up "delay 20ms rate $rate limit 50"
		mark "link 2 capacity $rate"
		sleep 8
	done
	sleep 13
}
