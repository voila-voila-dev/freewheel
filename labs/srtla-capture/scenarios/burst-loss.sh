# Bursty loss on link 2 (Gilbert-Elliott: rare bad periods losing most packets), like fading.
LINKS=2
run() {
	shape_link 2 both "delay 20ms loss gemodel 1% 10% 70% 0.1%"
	start_all 30 1 2
	sleep 33
}
