# Link 2 is a 2 Mbit/s modem with a huge buffer: latency grows instead of loss.
LINKS=2
run() {
	shape_link 2 up "delay 20ms rate 2mbit limit 5000"
	start_all 30 1 2
	sleep 33
}
