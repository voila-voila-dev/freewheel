# A stranger who sniffed the group id on the cellular network joins the group from its own address.
LINKS=3
run() {
	start_eavesdropping
	start_all 40 1 2
	sleep 5
	local stranger=10.3.0.2 group_id
	group_id=$(eavesdropped_group_id)
	mark "eavesdropped a ${#group_id}-hex-digit group id"
	send_hex "$stranger" 42000 "9201$group_id"
	mark "stranger registers into the group"
	for _ in $(seq 1 10); do
		send_hex "$stranger" 42000 "9000"
		sleep 1
	done
	mark "stranger injects data"
	for _ in $(seq 1 50); do
		send_hex "$stranger" 42000 "$(printf '%08x' "$RANDOM")$(random_hex 1312)"
	done
	mark "stranger stops"
	sleep 20
}
