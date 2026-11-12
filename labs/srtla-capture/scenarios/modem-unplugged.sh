# Modem 2 disappears for 10 s (sends fail at once, like a USB modem resetting), then comes back.
LINKS=2
run() {
	start_all 35 1 2
	sleep 10
	unplug_modem 2
	sleep 10
	plug_modem 2
	sleep 18
}
