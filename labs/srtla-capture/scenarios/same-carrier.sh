# Two SIMs of the same operator: links 1 and 2 reach the server from one public address, two ports.
LINKS=2
run() {
	shared_carrier_nat 1 1 2
	start_all 30 1 2
	sleep 33
}
