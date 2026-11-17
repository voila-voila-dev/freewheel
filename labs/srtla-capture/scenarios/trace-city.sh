# 60 s in town: two uplinks replay recorded LTE traces (Mahimahi), 5 and 8–14 Mbit/s, no dead zone: latency when the network is fine.
LINKS=2
run() {
	replay_trace 1 mahimahi/Verizon-LTE-short.up 0 70
	replay_trace 2 mahimahi/TMobile-LTE-short.up 0 70
	start_all 60 1 2
	sleep 63
}
