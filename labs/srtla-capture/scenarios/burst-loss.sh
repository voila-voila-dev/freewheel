# Bursty loss on link 2 (Gilbert-Elliott: rare bad periods losing most packets), like fading.
# The loss starts once the stream runs: a bad period over link 2's registration delays it by
# seconds (srtla.md, "Fragile start"), which is not what this measures.
LINKS=2
run() {
	start_all 30 1 2
	shape_link 2 both "delay 20ms loss gemodel 1% 10% 70% 0.1%"
	mark "link 2 fades"
	sleep 33
}
