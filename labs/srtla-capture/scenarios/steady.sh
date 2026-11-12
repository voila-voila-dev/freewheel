# Two equal links for 30 s: registration, ACKs, how traffic is split.
LINKS=2
run() {
	start_all 30 1 2
	sleep 33
}
