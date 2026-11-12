# The carrier NAT on link 2 changes its public port mid-stream (common on 4G CGNAT).
LINKS=2
run() {
	carrier_nat 2 20000
	start_all 40 1 2
	sleep 12
	carrier_nat 2 30000
	sleep 30
}
