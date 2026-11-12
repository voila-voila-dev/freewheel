# Malformed and unauthorised packets from a stranger, while the real stream runs.
LINKS=3
run() {
	start_all 35 1 2
	sleep 5
	local stranger=10.3.0.2
	send_hex "$stranger" 41000 "90"
	mark "1-byte packet"
	send_hex "$stranger" 41001 "9200$(random_hex 255)"
	mark "REG1 one byte short"
	send_hex "$stranger" 41002 "9201$(random_hex 256)"
	mark "REG2 with an unknown group id"
	send_hex "$stranger" 41003 "9000"
	mark "keepalive from an unregistered address"
	send_hex "$stranger" 41004 "00000001$(random_hex 100)"
	mark "data packet from an unregistered address"
	send_hex "$stranger" 41005 "9202"
	mark "REG3 sent to the receiver"
	send_hex "$stranger" 41006 "9200$(random_hex 256)"
	sleep 0.2
	send_hex "$stranger" 41006 "9200$(random_hex 256)"
	mark "REG1 twice from the same address"
	sleep 28
}
