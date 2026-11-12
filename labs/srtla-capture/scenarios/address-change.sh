# Modem 2 gets a new IP mid-stream; the box rewrites its link list and sends SIGHUP.
LINKS=2
run() {
	start_all 40 1 2
	sleep 12
	ip -n box addr del 10.2.0.2/24 dev m2
	ip -n box addr add 10.2.0.3/24 dev m2
	ip -n box rule del from 10.2.0.2 table 2
	ip -n box rule add from 10.2.0.3 table 2
	ip -n box route replace default via 10.2.0.1 dev m2 table 2
	mark "modem 2 now has 10.2.0.3"
	write_sender_ips 1 1
	echo 10.2.0.3 >> /tmp/srtla_ips.1
	in_box pkill -HUP -f "$SENDER_NAME 6001"
	mark "sender 1 reloaded with 10.1.0.2 10.2.0.3"
	sleep 30
}
