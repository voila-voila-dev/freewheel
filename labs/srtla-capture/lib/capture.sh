# Two captures of SRTLA traffic: what the box sends and receives (before the network), and
# what the server sees (after it).
#
# CAPTURE_SNAPLEN (bytes kept per frame, 0 = all) lets long scenarios keep headers only.

start_captures() {
	in_box tcpdump -i any -n -U -s "${CAPTURE_SNAPLEN:-0}" -w "$OUT/$SCENARIO.box.pcap" "udp port $SRTLA_PORT" \
		2>> "$OUT/$SCENARIO.tcpdump.log" &
	in_server tcpdump -i any -n -U -s "${CAPTURE_SNAPLEN:-0}" -w "$OUT/$SCENARIO.server.pcap" "udp port $SRTLA_PORT" \
		2>> "$OUT/$SCENARIO.tcpdump.log" &
	sleep 1
}

stop_captures() {
	sleep 1
	pkill -INT tcpdump || true
	sleep 0.5
}
