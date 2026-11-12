# 200 fake groups registered before the real box (the receiver's limit): the box is refused.
LINKS=2
run() {
	start_srt_listener
	start_receiver
	local senders=()
	for port in $(seq 40000 40199); do
		send_hex 10.2.0.2 "$port" "9200$(random_hex 256)" &
		senders+=($!)
	done
	wait "${senders[@]}"
	mark "200 fake groups registered"
	start_sender 1 1
	start_stream 1 30
	sleep 33
}
