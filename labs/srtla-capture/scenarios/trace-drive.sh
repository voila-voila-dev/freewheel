# 90 s of a real drive: three uplinks replay recorded LTE traces (Mahimahi), 2.4, 0.9 and 2.0 Mbit/s on average with dead seconds.
LINKS=3
run() {
	replay_trace 1 mahimahi/Verizon-LTE-driving.up 240 100
	replay_trace 2 mahimahi/ATT-LTE-driving.up 210 100
	replay_trace 3 mahimahi/ATT-LTE-driving-2016.up 0 100
	start_all 90 1 2 3
	sleep 93
}
