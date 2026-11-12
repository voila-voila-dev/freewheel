# 5 % random loss on link 2, both directions: NAKs and how the window reacts.
LINKS=2
run() {
	shape_link 2 both "delay 20ms loss 5%"
	start_all 30 1 2
	sleep 33
}
