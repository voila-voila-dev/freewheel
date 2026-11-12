# Impairments on link k, applied in the net namespace with tc netem.
# Direction: up (box → server), down (server → box) or both.

interfaces_for() {
	local k=$1 direction=$2
	case "$direction" in
		up) echo "s$k" ;;
		down) echo "b$k" ;;
		both) echo "s$k b$k" ;;
	esac
}

# shape_link <k> <direction> "<netem arguments>", e.g. shape_link 2 up "delay 80ms rate 2mbit"
shape_link() {
	local k=$1 direction=$2 netem=$3
	for interface in $(interfaces_for "$k" "$direction"); do
		# shellcheck disable=SC2086 # netem arguments are words on purpose
		in_net tc qdisc replace dev "$interface" root netem $netem
	done
}

# cut_link <k> [direction]: packets vanish silently, like a dead zone.
cut_link() {
	shape_link "$1" "${2:-both}" "loss 100%"
	mark "cut link $1 ${2:-both}"
}

# restore_link <k> [direction]: back to the default delay.
restore_link() {
	shape_link "$1" "${2:-both}" "delay $DEFAULT_ONE_WAY_DELAY"
	mark "restore link $1 ${2:-both}"
}

# unplug_modem <k> / plug_modem <k>: the interface goes away, so sends fail at once.
unplug_modem() {
	ip -n box link set "m$1" down
	mark "unplug modem $1"
}

plug_modem() {
	ip -n box link set "m$1" up
	ip -n box route replace default via "10.$1.0.1" dev "m$1" table "$1"
	mark "plug modem $1"
}

# carrier_nat <k> <first-port>: the operator's NAT rewrites the source port of link k.
# Calling it again with another port flushes the NAT table: the link changes port.
carrier_nat() {
	local k=$1 first_port=$2
	in_net iptables -t nat -F POSTROUTING
	in_net iptables -t nat -A POSTROUTING -o "s$k" -p udp \
		-j SNAT --to-source "10.100.$k.1:$first_port-$((first_port + 999))"
	in_net conntrack -D -p udp >/dev/null 2>&1 || true
	mark "carrier NAT on link $k from port $first_port"
}

# shared_carrier_nat <public-k> <k…>: links k… are SIMs of one operator and leave through link
# public-k's public address, each on its own port range (one CGNAT, several modems).
shared_carrier_nat() {
	local public_k=$1 first_port=20000
	shift
	in_net iptables -t nat -F POSTROUTING
	for k in "$@"; do
		in_net iptables -t nat -A POSTROUTING -o "s$k" -p udp \
			-j SNAT --to-source "10.100.$public_k.1:$first_port-$((first_port + 999))"
		first_port=$((first_port + 1000))
	done
	in_net conntrack -D -p udp >/dev/null 2>&1 || true
	mark "links $* share the public address of link $public_k"
}
