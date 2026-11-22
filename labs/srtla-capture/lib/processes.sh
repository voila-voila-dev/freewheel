# Starting and stopping the SRT stream, the sender and the receiver.
#
#   box:    ffmpeg ─udp:700<s>─▶ srt-live-transmit ─▶ sender :600<s> ─▶ links
#   server: receiver :5000 ─▶ srt-live-transmit listener :5002 ─▶ <scenario>.ts
#
# RECEIVER=upstream (default) runs upstream srtla_rec; RECEIVER=freewheel runs our
# bonding-receiver. SENDER=upstream (default) runs upstream srtla_send; SENDER=freewheel runs our
# bonding-sender; SENDER=irlserver runs irlserver/srtla_send (bench image only). RECEIVER=transport
# and SENDER=transport run our own transport (M1b candidate: freewheel-receiver, freewheel-sender),
# which only talk to each other. run-all.sh builds ours and mounts them at /opt/freewheel. Same
# command lines.

readonly SRTLA=/opt/srtla
case "${SENDER:-upstream}" in
	upstream) readonly SENDER_PROGRAM=$SRTLA/srtla_send ;;
	freewheel) readonly SENDER_PROGRAM=/opt/freewheel/bonding-sender ;;
	irlserver) readonly SENDER_PROGRAM=/opt/irlserver/srtla_send ;;
	transport) readonly SENDER_PROGRAM=/opt/freewheel/freewheel-sender ;;
	*)
		echo "unknown SENDER=$SENDER (upstream, freewheel, irlserver or transport)" >&2
		exit 2
		;;
esac
# What pkill -f matches: the program name followed by its listen port.
SENDER_NAME=$(basename "$SENDER_PROGRAM")
readonly SENDER_NAME
case "${RECEIVER:-upstream}" in
	upstream) readonly RECEIVER_PROGRAM=$SRTLA/srtla_rec ;;
	freewheel) readonly RECEIVER_PROGRAM=/opt/freewheel/bonding-receiver ;;
	transport) readonly RECEIVER_PROGRAM=/opt/freewheel/freewheel-receiver ;;
	*)
		echo "unknown RECEIVER=$RECEIVER (upstream, freewheel or transport)" >&2
		exit 2
		;;
esac
# ENCODER=fixed: ffmpeg's libx264 at $BITRATE, whatever the links do. ENCODER=adaptive (the
# default with SENDER=transport, M1b step 5): our lab-encoder (crates/h264-encoder), whose bitrate
# follows the capacity reports freewheel-sender sends to $CAPACITY_SOCKET_PREFIX.<s>.sock, with
# $BITRATE plus the audio's share as its ceiling (no audio, see start_stream). Either way ffmpeg
# makes the test pattern and the MPEG-TS. LAB_ENCODER_LOG_REPORTS=1 also marks every report.
if [ "${SENDER:-upstream}" = transport ]; then ENCODER=${ENCODER:-adaptive}; else ENCODER=${ENCODER:-fixed}; fi
case "$ENCODER" in
	fixed | adaptive) readonly ENCODER ;;
	*)
		echo "unknown ENCODER=$ENCODER (fixed or adaptive)" >&2
		exit 2
		;;
esac
readonly LAB_ENCODER=/opt/freewheel/lab-encoder
readonly CAPACITY_SOCKET_PREFIX=/tmp/freewheel-capacity
# The fixed encoder's audio, which the adaptive one gives to its video (start_stream).
readonly AUDIO_BPS=128000
# Dates each video frame on both ends (crates/packet-capture, `frame-times`), mounted by run-all.sh.
readonly FRAME_TIMES=/opt/freewheel/frame-times
readonly STREAM_PORT_BASE=7000
readonly SRTLA_PORT=5000
readonly SRT_LISTENER_PORT=5002
readonly SRT_LATENCY_MS=2000
BITRATE=${BITRATE:-3M}

# The listener's output goes through frame-times (when mounted), which appends it to
# <scenario>.ts and dates each frame as it is delivered: <scenario>.received.times.
start_srt_listener() {
	if [ -x "$FRAME_TIMES" ]; then
		in_server srt-live-transmit -q -s:1000 -pf:json -f -statsout:"$OUT/$SCENARIO.srt-stats.json" \
			"srt://127.0.0.1:$SRT_LISTENER_PORT?mode=listener&lossmaxttl=40&latency=$SRT_LATENCY_MS" \
			"file://con" 2>> "$OUT/$SCENARIO.srt-listener.log" |
			"$FRAME_TIMES" record "$OUT/$SCENARIO.ts" "$OUT/$SCENARIO.received.times" &
	else
		in_server srt-live-transmit -q -s:1000 -pf:json -f -statsout:"$OUT/$SCENARIO.srt-stats.json" \
			"srt://127.0.0.1:$SRT_LISTENER_PORT?mode=listener&lossmaxttl=40&latency=$SRT_LATENCY_MS" \
			"file://con" >> "$OUT/$SCENARIO.ts" 2>> "$OUT/$SCENARIO.srt-listener.log" &
	fi
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
		FREEWHEEL_CAPACITY_SOCKET="$CAPACITY_SOCKET_PREFIX.$sender.sock" \
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
	local outputs="[f=mpegts]udp://127.0.0.1:$((7000 + sender))?pkt_size=1316|[f=mpegts]$OUT/$SCENARIO.sent.$sender.ts"
	if [ "$ENCODER" = adaptive ]; then
		# Raw frames → lab-encoder (its bitrate marks go to the marks file) → ffmpeg puts it in
		# MPEG-TS. The muxer reads the video by the wall clock (a raw H.264 pipe has no
		# timestamps: otherwise it holds the video back ~25 s), then setts numbers the frames at
		# 30 per second (two frames read in the same millisecond would share a timestamp, which
		# MPEG-TS refuses). No audio: muxed with this pipe, ffmpeg sends the video in bursts with
		# 100–250 ms holes, ~3 a second, which SRT turns into late frames. The video may take the
		# audio's 128 kbit/s instead, so the stream weighs the same on the links.
		local ceiling_bps=$(($(bitrate_bps "$BITRATE") + AUDIO_BPS))
		in_box sh -c "ffmpeg -loglevel error -re -f lavfi -i testsrc2=size=1280x720:rate=30 -t $seconds -f rawvideo -pix_fmt yuv420p - \
			| $LAB_ENCODER --width 1280 --height 720 --fps 30 --keyint 60 --max-bitrate $ceiling_bps \
				--capacity-socket $CAPACITY_SOCKET_PREFIX.$sender.sock 2>> $OUT/$SCENARIO.marks \
			| ffmpeg -loglevel error -probesize 100000 -analyzeduration 0 -f h264 -framerate 30 \
				-use_wallclock_as_timestamps 1 -i - -map 0:v -c:v copy -bsf:v 'setts=ts=N/(30*TB)' -t $seconds \
				-f tee '$outputs'" \
			2>> "$OUT/$SCENARIO.ffmpeg.log" &
	else
		in_box ffmpeg -loglevel error -re -f lavfi -i "testsrc2=size=1280x720:rate=30" \
			-f lavfi -i "sine=frequency=440" -t "$seconds" \
			-c:v libx264 -preset ultrafast -tune zerolatency \
			-b:v "$BITRATE" -maxrate "$BITRATE" -bufsize "$BITRATE" -g 60 \
			-c:a aac -b:a 128k -map 0:v -map 1:a -f tee "$outputs" \
			2>> "$OUT/$SCENARIO.ffmpeg.log" &
	fi
	mark "stream $sender started at $BITRATE for ${seconds}s"
	mark "stream $sender encoder $ENCODER"
}

# bitrate_bps <rate>: 3M, 800k or 2500000 → bit/s.
bitrate_bps() {
	case "$1" in
		*M) echo $((${1%M} * 1000000)) ;;
		*k) echo $((${1%k} * 1000)) ;;
		*) echo "$1" ;;
	esac
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

# start_raw_stream <sender> <seconds> <bits per second>: random bytes at a fixed rate through SRT,
# for throughput: no encoder to pace or to load the CPU. SRT payloads of 1316 bytes, like MPEG-TS.
start_raw_stream() {
	local sender=$1 seconds=$2 rate=$3
	local caller_log="$OUT/$SCENARIO.srt-caller.$sender.log"
	local connections
	connections=$(srt_connections "$caller_log")
	# The caller appends to its log while the subshell waits for it to say it connected.
	# shellcheck disable=SC2094
	(
		# Marks go to stderr here: stdout is the stream.
		wait_for_srt_connection "$caller_log" "$connections" "$sender" >&2
		head -c "$((rate * seconds / 8))" /dev/urandom | pv -q -L "$((rate / 8))" -B 65536
	) | in_box srt-live-transmit -chunk:1316 file://con \
		"srt://127.0.0.1:$((6000 + sender))?latency=$SRT_LATENCY_MS&maxbw=0" \
		>> "$caller_log" 2>&1 &
	mark "raw stream $sender started at $rate bit/s for ${seconds}s"
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
	sample_resources
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

# sample_resources: CPU time and peak resident memory of the sender and the receiver, since they
# started, marked as "resources <role> cpu_ms=<n> peak_kb=<n>" (summed over their processes).
sample_resources() {
	mark "resources sender $(process_resources "$SENDER_PROGRAM")"
	mark "resources receiver $(process_resources "$RECEIVER_PROGRAM")"
}

# process_resources <binary>: "cpu_ms=<user + system> peak_kb=<VmHWM>" of the processes running it.
process_resources() {
	local ticks=0 peak_kb=0 pid stat hwm
	local hertz
	hertz=$(getconf CLK_TCK)
	for pid in $(pgrep -f "^$1( |$)" || true); do
		stat=$(cut -d')' -f2- "/proc/$pid/stat" 2> /dev/null) || continue
		# After "pid (comm)": field 12 is utime, 13 stime (14 and 15 of the whole line).
		ticks=$((ticks + $(echo "$stat" | awk '{ print $12 + $13 }')))
		hwm=$(awk '/^VmHWM:/ { print $2 }' "/proc/$pid/status" 2> /dev/null || echo 0)
		peak_kb=$((peak_kb + ${hwm:-0}))
	done
	echo "cpu_ms=$((ticks * 1000 / hertz)) peak_kb=$peak_kb"
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

# write_frame_times: when each frame of stream <s> left the encoder (<scenario>.sent.<s>.times),
# from the capture of its UDP output, which is then deleted (it holds the whole video).
# The delivery side is written live by the listener (<scenario>.received.times).
write_frame_times() {
	local sent sender
	if [ -x "$FRAME_TIMES" ] && [ -f "$OUT/$SCENARIO.stream.pcap" ]; then
		for sent in "$OUT/$SCENARIO".sent.*.ts; do
			[ -f "$sent" ] || continue
			sender=${sent%.ts}
			sender=${sender##*.}
			"$FRAME_TIMES" pcap "$OUT/$SCENARIO.stream.pcap" "$((STREAM_PORT_BASE + sender))" > "${sent%.ts}.times" || true
		done
	fi
	rm -f "$OUT/$SCENARIO.stream.pcap"
}

frame_digests() {
	ffmpeg -loglevel quiet -err_detect ignore_err -copyts -i "$1" -map 0:v -c copy -f framemd5 - || true
}
