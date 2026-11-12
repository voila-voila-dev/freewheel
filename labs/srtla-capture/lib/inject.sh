# Hand-made packets, sent from the box on an address that is not a registered link.

# send_hex <source-ip> <source-port> <hex bytes>: one datagram to the receiver.
send_hex() {
	local source_ip=$1 source_port=$2 hex=$3
	echo -n "$hex" | xxd -r -p \
		| in_box nc -u -w1 -q0 -s "$source_ip" -p "$source_port" "$RECEIVER_IP" "$SRTLA_PORT" || true
}

# random_hex <bytes>
random_hex() {
	head -c "$1" /dev/urandom | xxd -p | tr -d '\n'
}

# start_eavesdropping / eavesdropped_group_id: what someone sniffing the cellular network
# learns, the full group id in the receiver's REG2 answer. Start before the sender registers.
start_eavesdropping() {
	in_net tcpdump -i any -y LINUX_SLL2 -n -c 1 -w /tmp/eavesdropped.pcap \
		"udp src port $SRTLA_PORT and udp[8:2] = 0x9201" 2>/dev/null &
	sleep 0.5
}

eavesdropped_group_id() {
	# pcap header 24 + record header 16 + Linux cooked v2 20 + IPv4 20 + UDP 8 + SRTLA type 2 = 90.
	tail -c +91 /tmp/eavesdropped.pcap | head -c 256 | xxd -p | tr -d '\n'
}
