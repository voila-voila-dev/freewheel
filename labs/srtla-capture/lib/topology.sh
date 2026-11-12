# Network namespaces: box (the sender, one interface per modem), net (the cellular networks,
# where impairments live) and server (the receiver).
#
#   box  m<k> 10.<k>.0.2 ── b<k> 10.<k>.0.1  net  s<k> 10.100.<k>.1 ── n<k> 10.100.<k>.2  server
#                                                                     receiver: 10.200.0.1
#
# The server answers from its public address on every link, like a real server, so carrier NAT
# can match replies.
#
# Link k: box → server traffic leaves through m<k> (source routing, like a real box) and crosses
# net, where s<k> carries the uplink impairment and b<k> the downlink one.

readonly RECEIVER_IP=10.200.0.1
readonly DEFAULT_ONE_WAY_DELAY=20ms

in_box() { ip netns exec box "$@"; }
in_net() { ip netns exec net "$@"; }
in_server() { ip netns exec server "$@"; }

create_namespaces() {
	for namespace in box net server; do
		ip netns add "$namespace"
		ip -n "$namespace" link set lo up
		ip netns exec "$namespace" sysctl -qw net.ipv4.conf.all.rp_filter=0 net.ipv4.conf.default.rp_filter=0
	done
	in_net sysctl -qw net.ipv4.ip_forward=1
	ip -n server addr add "$RECEIVER_IP/32" dev lo
}

# create_link <k>: one modem, from the box to the server through net.
create_link() {
	local k=$1
	ip link add "m$k" netns box type veth peer name "b$k" netns net
	ip link add "s$k" netns net type veth peer name "n$k" netns server
	ip -n box addr add "10.$k.0.2/24" dev "m$k"
	ip -n net addr add "10.$k.0.1/24" dev "b$k"
	ip -n net addr add "10.100.$k.1/24" dev "s$k"
	ip -n server addr add "10.100.$k.2/24" dev "n$k"
	local namespace interface
	for pair in "box:m$k" "net:b$k" "net:s$k" "server:n$k"; do
		namespace=${pair%%:*}
		interface=${pair##*:}
		ip -n "$namespace" link set "$interface" up
		ip netns exec "$namespace" sysctl -qw "net.ipv4.conf.$interface.rp_filter=0"
	done
	ip -n box rule add from "10.$k.0.2" table "$k"
	ip -n box route add default via "10.$k.0.1" dev "m$k" table "$k"
	ip -n net rule add from "10.$k.0.0/24" table "$k"
	ip -n net route add "$RECEIVER_IP" via "10.100.$k.2" dev "s$k" table "$k"
	ip -n server route add "10.$k.0.0/24" via "10.100.$k.1" dev "n$k" src "$RECEIVER_IP"
	ip -n server route replace "10.100.$k.0/24" dev "n$k" src "$RECEIVER_IP"
	shape_link "$k" both "delay $DEFAULT_ONE_WAY_DELAY"
}

# create_links <count>
create_links() {
	for k in $(seq 1 "$1"); do create_link "$k"; done
}

# link_ip <k>: the box's address on modem k.
link_ip() { echo "10.$1.0.2"; }
