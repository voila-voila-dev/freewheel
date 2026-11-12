# Two minutes of a real ride: three uneven, lossy, jittery modems, a dead zone, a handover, a weak cell.
LINKS=3
run() {
	shape_link 1 up "delay 30ms 10ms rate 6mbit limit 100 loss 0.5%"
	shape_link 2 up "delay 50ms 20ms rate 3mbit limit 100 loss gemodel 1% 20%"
	shape_link 3 up "delay 80ms 30ms rate 2mbit limit 100 loss 2%"
	start_all 120 1 2 3
	sleep 20
	cut_link 1
	sleep 8
	shape_link 1 both "delay 30ms 10ms rate 6mbit limit 100 loss 0.5%"
	mark "link 1 back"
	sleep 17
	shape_link 2 both "delay 1000ms"
	mark "handover on link 2"
	sleep 4
	shape_link 2 up "delay 50ms 20ms rate 3mbit limit 100 loss gemodel 1% 20%"
	shape_link 2 down "delay $DEFAULT_ONE_WAY_DELAY"
	mark "link 2 back"
	sleep 16
	shape_link 3 up "delay 80ms 30ms rate 500kbit limit 50 loss 2%"
	mark "weak cell on link 3"
	sleep 15
	shape_link 3 up "delay 80ms 30ms rate 2mbit limit 100 loss 2%"
	mark "link 3 back"
	sleep 10
	cut_link 1
	cut_link 2
	sleep 4
	shape_link 1 both "delay 30ms 10ms rate 6mbit limit 100 loss 0.5%"
	shape_link 2 up "delay 50ms 20ms rate 3mbit limit 100 loss gemodel 1% 20%"
	shape_link 2 down "delay $DEFAULT_ONE_WAY_DELAY"
	mark "links 1 and 2 back"
	sleep 30
}
