# Four equal links: how the scheduler spreads traffic over more modems.
LINKS=4
run() {
	start_all 30 1 2 3 4
	sleep 33
}
