# 5 % random loss on link 2, both directions: NAKs and how the window reacts.
# The loss starts once the stream runs: a single lost registration or SRT handshake answer
# stalls the start by 5 to 20 s (srtla.md, "Known defects"), which is not what this measures.
LINKS=2
run() {
	start_all 30 1 2
	shape_link 2 both "delay 20ms loss 5%"
	mark "link 2 loses 5 %"
	sleep 33
}
