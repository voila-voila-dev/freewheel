# Three unequal links (8 Mbit/s 30 ms, 2 Mbit/s 80 ms, 0.5 Mbit/s 200 ms): does each get its share?
LINKS=3
run() {
	shape_link 1 up "delay 30ms rate 8mbit limit 200"
	shape_link 2 up "delay 80ms rate 2mbit limit 200"
	shape_link 3 up "delay 200ms rate 500kbit limit 200"
	start_all 30 1 2 3
	sleep 33
}
