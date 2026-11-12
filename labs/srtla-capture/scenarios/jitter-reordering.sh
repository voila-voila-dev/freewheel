# Link 2 with 60 ± 40 ms jitter and reordering: out-of-order arrival across and within links.
LINKS=2
run() {
	shape_link 2 up "delay 60ms 40ms distribution normal reorder 10% 50%"
	start_all 30 1 2
	sleep 33
}
