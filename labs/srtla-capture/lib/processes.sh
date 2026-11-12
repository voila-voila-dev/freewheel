# Starting and stopping the SRT stream, the sender and the receiver.
#
#   box:    ffmpeg ─udp:700<s>─▶ srt-live-transmit ─▶ sender :600<s> ─▶ links
#   server: receiver :5000 ─▶ srt-live-transmit listener :5002 ─▶ <scenario>.ts
#
# RECEIVER=upstream (default) runs upstream srtla_rec; RECEIVER=freewheel runs our
# bonding-receiver. SENDER=upstream (default) runs upstream srtla_send; SENDER=freewheel runs our
# bonding-sender. run-all.sh builds ours and mounts them at /opt/freewheel. Same command lines.

readonly SRTLA=/opt/srtla
case "${SENDER:-upstream}" in
	upstream) readonly SENDER_PROGRAM=$SRTLA/srtla_send ;;
	freewheel) readonly SENDER_PROGRAM=/opt/freewheel/bonding-sender ;;
	*)
		echo "unknown SENDER=$SENDER (upstream or freewheel)" >&2
		exit 2
		;;
esac
# What pkill -f matches: the program name followed by its listen port.
SENDER_NAME=$(basename "$SENDER_PROGRAM")
readonly SENDER_NAME
case "${RECEIVER:-upstream}" in
	upstream) readonly RECEIVER_PROGRAM=$SRTLA/srtla_rec ;;
	freewheel) readonly RECEIVER_PROGRAM=/opt/freewheel/bonding-receiver ;;
	*)
		echo "unknown RECEIVER=$RECEIVER (upstream or freewheel)" >&2
		exit 2
		;;
esac
readonly SRTLA_PORT=5000
readonly SRT_LISTENER_PORT=5002
readonly SRT_LATENCY_MS=2000
BITRATE=${BITRATE:-3M}

start_srt_listener() {
	in_server srt-live-transmit -q -s:1000 -pf:json -f -statsout:"$OUT/$SCENARIO.srt-stats.json" \
		"srt://127.0.0.1:$SRT_LISTENER_PORT?mode=listener&lossmaxttl=40&latency=$SRT_LATENCY_MS" \
		"file://con" >> "$OUT/$SCENARIO.ts" 2>> "$OUT/$SCENARIO.srt-listener.log" &
	sleep 0.5
}

stop_srt_listener() {
	in_server pkill -f "mode=listener" || true
	mark "SRT listener stopped"
}

start_receiver() {
	in_server "$RECEIVER_PROGRAM" "$SRTLA_PORT" 127.0.0.1 "$SRT_LISTENER_PORT" 2>> "$OUT/$SCENARIO.receiver.log" &
	sleep 0.3
	mark "receiver started"
}

stop_receiver() {
	in_server pkill -f "$RECEIVER_PROGRAM" || true
	mark "receiver stopped"
}

# write_sender_ips <sender> <k…>: the links a sender uses (reread on SIGHUP).
write_sender_ips() {
	local sender=$1
	shift
	: > "/tmp/srtla_ips.$sender"
	for k in "$@"; do link_ip "$k" >> "/tmp/srtla_ips.$sender"; done
}

# start_sender <sender> <k…>: the sender on the given links; its exit is marked.
start_sender() {
	local sender=$1
	shift
	write_sender_ips "$sender" "$@"
	(
		status=0
		in_box "$SENDER_PROGRAM" "$((6000 + sender))" "$RECEIVER_IP" "$SRTLA_PORT" "/tmp/srtla_ips.$sender" \
			>> "$OUT/$SCENARIO.sender.$sender.log" 2>&1 || status=$?
		mark "sender $sender exited with $status"
	) &
	sleep 0.3
	mark "sender $sender started on links $*"
}

stop_sender() {
	in_box pkill -f "$SENDER_NAME $((6000 + $1))" || true
	mark "sender $1 stopped"
}

# reload_sender_links <sender> <k…>: rewrite the link list and send SIGHUP, as belaUI does.
reload_sender_links() {
	local sender=$1
	shift
	write_sender_ips "$sender" "$@"
	in_box pkill -HUP -f "$SENDER_NAME $((6000 + sender))"
	mark "sender $sender reloaded with links $*"
}

# start_stream <sender> <seconds> [now]: test pattern → SRT caller → sender. The encoder starts
# once the SRT caller is connected, so the run does not depend on how fast the handshake was:
# bytes sent before that are dropped, and with them the first keyframe and its whole GOP. `now`
# starts it at once, for scenarios about video lost before the connection (cold-start).
# The encoder also keeps what it sent in <scenario>.sent.<sender>.ts, to check the received
# video frame by frame.
start_stream() {
	local sender=$1 seconds=$2 when=${3:-connected}
	local caller_log="$OUT/$SCENARIO.srt-caller.$sender.log"
	local connections
	connections=$(srt_connections "$caller_log")
	in_box srt-live-transmit "udp://127.0.0.1:$((7000 + sender))" \
		"srt://127.0.0.1:$((6000 + sender))?latency=$SRT_LATENCY_MS" \
		>> "$caller_log" 2>&1 &
	if [ "$when" = connected ]; then
		wait_for_srt_connection "$caller_log" "$connections" "$sender"
	fi
	in_box ffmpeg -loglevel error -re -f lavfi -i "testsrc2=size=1280x720:rate=30" \
		-f lavfi -i "sine=frequency=440" -t "$seconds" \
		-c:v libx264 -preset ultrafast -tune zerolatency \
		-b:v "$BITRATE" -maxrate "$BITRATE" -bufsize "$BITRATE" -g 60 \
		-c:a aac -b:a 128k -map 0:v -map 1:a -f tee \
		"[f=mpegts]udp://127.0.0.1:$((7000 + sender))?pkt_size=1316|[f=mpegts]$OUT/$SCENARIO.sent.$sender.ts" \
		2>> "$OUT/$SCENARIO.ffmpeg.log" &
	mark "stream $sender started at $BITRATE for ${seconds}s"
}

readonly SRT_CONNECTED="SRT target connected"
readonly SRT_CONNECT_TIMEOUT_TENTHS=100

# srt_connections <log>: how many times an SRT caller logged a connection.
srt_connections() {
	local count
	count=$(grep -c "$SRT_CONNECTED" "$1" 2> /dev/null || true)
	echo "${count:-0}"
}

# wait_for_srt_connection <log> <connections before> <sender>: up to 10 s for one more.
wait_for_srt_connection() {
	local log=$1 before=${2:-0} sender=$3 tenths=0
	while [ "$(srt_connections "$log")" -le "$before" ]; do
		if [ "$tenths" -ge "$SRT_CONNECT_TIMEOUT_TENTHS" ]; then
			mark "SRT caller $sender not connected after 10 s, encoder started anyway"
			return
		fi
		sleep 0.1
		tenths=$((tenths + 1))
	done
	mark "SRT caller $sender connected"
}

# start_all <seconds> <k…>: SRT listener, receiver, sender 1 on the given links, stream.
start_all() {
	local seconds=$1
	shift
	start_srt_listener
	start_receiver
	start_sender 1 "$@"
	start_stream 1 "$seconds"
}

stop_all() {
	in_box pkill -f ffmpeg || true
	sleep 1
	in_box pkill -f "$SENDER_PROGRAM" || true
	in_box pkill -f srt-live-transmit || true
	in_server pkill -f "$RECEIVER_PROGRAM" || true
	in_server pkill -f srt-live-transmit || true
}

# sample_memory: marks the resident memory of the sender and the receiver, in kB.
sample_memory() {
	mark "memory sender $(resident_kb "$SENDER_PROGRAM")"
	mark "memory receiver $(resident_kb "$RECEIVER_PROGRAM")"
}

# resident_kb <binary>: total resident memory of the processes running it, in kB.
resident_kb() {
	local pids
	pids=$(pgrep -d, -f "$1" || true)
	if [ -z "$pids" ]; then
		echo 0
		return
	fi
	ps -o rss= -p "$pids" | awk '{ total += $1 } END { print total + 0 }'
}

# write_frame_digests: one MD5 per encoded video frame, keyed by its timestamp, of what each
# stream sent and of what the SRT listener received. Hashing the compressed frames, not the
# decoded pictures, tells intact frames from damaged ones exactly. -copyts keeps the stream's own
# timestamps: otherwise each file is shifted to its own start and a late first packet offsets them all.
write_frame_digests() {
	local sent
	for sent in "$OUT/$SCENARIO".sent.*.ts; do
		[ -f "$sent" ] || continue
		frame_digests "$sent" > "${sent%.ts}.frames"
	done
	: > "$OUT/$SCENARIO.received.frames"
	if [ -s "$OUT/$SCENARIO.ts" ]; then
		frame_digests "$OUT/$SCENARIO.ts" > "$OUT/$SCENARIO.received.frames"
	fi
}

frame_digests() {
	ffmpeg -loglevel quiet -err_detect ignore_err -copyts -i "$1" -map 0:v -c copy -f framemd5 - || true
}
