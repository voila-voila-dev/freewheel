# Link emulator

`crates/link-emulator` reproduces a day of cellular connectivity on a laptop or in CI,
deterministically: the same seed gives the same packets, losses and delays everywhere.

## What it models

| Piece | Type | Real-world effect |
|---|---|---|
| Capacity over time | `Trace` (Mahimahi format) | The network gives more or less room every millisecond |
| Modem buffer | `LinkConfig::queue_bytes` | Packets wait in the modem (delay grows), then overflow |
| Propagation | `LinkConfig::propagation_ms` | Distance to the server |
| Bursty loss | `GilbertElliott` | Losses come in bursts: tunnels, hills, interference |
| Events | `Event` | `Outage`, `Handover` (cell change), `AddressChange`, `ModemReboot` |
| Both directions | `Path` | Uplink and downlink of one SIM, sharing events |
| Radio power | `RadioPower` | Active, tail and idle power states (milliwatts) |
| Data billing | `DataMeter` | Bytes up and down as the operator bills them, cost per GB |
| Shared cell | `Tower` | Two SIMs on the same mast degrade together |
| Synthetic traces | `RateModel` | New traces with the statistics of real ones |

## Ready-made scenarios

`scenarios::desert_day`, `saturated_city`, `mountain_tunnels`, `shared_tower`, or `scenarios::all(seed)`.

## Public traces

```sh
bun run traces:fetch   # Mahimahi LTE traces into .cache/traces (never committed)
cargo test --release -p link-emulator --test public_traces
cargo run --release -p link-emulator --bin synthesize_trace -- \
  .cache/traces/mahimahi/Verizon-LTE-short.up 60000 1 > synthetic.up
```

Public traces are only read; what we keep and share are synthetic traces derived from their statistics.

## Using it in a test

```rust
let mut path = Path::new(up_config, down_config, &events, RadioPower::lte_typical(), seed);
for now in 0..duration_ms {
    let (uplink, downlink) = path.advance(now, None);
    // feed `uplink` departures to the receiver under test, `downlink` to the sender
    path.send_up(packet_id, bytes, now);
}
let watts_hours = path.energy_mj() as f64 / 3_600_000.0;
let billed = path.meter().total_bytes();
```
