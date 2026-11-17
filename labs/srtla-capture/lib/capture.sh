# Two captures of SRTLA traffic: what the box sends and receives (before the network), and
# what the server sees (after it). A third, of the encoders' output, dates each video frame.
#
# CAPTURE_SNAPLEN (bytes kept per frame, 0 = all) lets long scenarios keep headers only.

start_captures() {
	in_box tcpdump -i any -n -U -s "${CAPTURE_SNAPLEN:-0}" -w "$OUT/$SCENARIO.box.pcap" "udp port $SRTLA_PORT" \
		2>> "$OUT/$SCENARIO.tcpdump.log" &
	in_server tcpdump -i any -n -U -s "${CAPTURE_SNAPLEN:-0}" -w "$OUT/$SCENARIO.server.pcap" "udp port $SRTLA_PORT" \
		2>> "$OUT/$SCENARIO.tcpdump.log" &
	# The encoders' output to their SRT caller, whole: write_frame_times dates each frame from it.
	in_box tcpdump -i lo -n -U -w "$OUT/$SCENARIO.stream.pcap" "udp dst portrange $((STREAM_PORT_BASE + 1))-$((STREAM_PORT_BASE + 9))" \
		2>> "$OUT/$SCENARIO.tcpdump.log" &
	sleep 1
}

stop_captures() {
	sleep 1
	pkill -INT tcpdump || true
	sleep 0.5
}
