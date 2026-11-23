# 90 s of a real drive: three uplinks replay recorded LTE traces (Mahimahi), 2.4, 0.9 and 2.0 Mbit/s on average with dead seconds.
LINKS=3
# Its links carry 3.96 Mbit/s once the traces end: the 6 Mbit/s recording (67.5 MB) needs about
# 125 s after the stream to arrive whole, so its links stay up to twice the stream (Emilien's
# choice, decisions.md 2026-11-23).
RECORDING_DRAIN_S=${RECORDING_DRAIN_S:-180}
run() {
	replay_trace 1 mahimahi/Verizon-LTE-driving.up 240 100
	replay_trace 2 mahimahi/ATT-LTE-driving.up 210 100
	replay_trace 3 mahimahi/ATT-LTE-driving-2016.up 0 100
	start_all 90 1 2 3
	sleep 93
}
