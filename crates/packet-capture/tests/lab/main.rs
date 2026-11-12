//! Characterisation of upstream SRTLA: one module per lab scenario (`labs/srtla-capture/`).
//!
//! Each test states what upstream does in that situation, measured on its captures. They run
//! when the lab has been run (`labs/srtla-capture/run-all.sh`) and are skipped otherwise; CI
//! runs the lab and requires them. Our receiver and sender will be checked against the same
//! scenarios: compatibility mode must match, our own transport must do better.

mod support;

mod address_change;
mod all_links_drop;
mod all_links_lost;
mod asymmetric_links;
mod bufferbloat;
mod burst_loss;
mod cold_start;
mod congested;
mod downlink_only_drop;
mod four_links;
mod group_hijack;
mod group_limit;
mod hostile_packets;
mod jitter_reordering;
mod latency_spike;
mod link_added;
mod link_drop;
mod link_drop_long;
mod link_flapping;
mod link_removed;
mod long_run;
mod low_bitrate;
mod modem_unplugged;
mod nat_rebinding;
mod random_loss;
mod real_ride;
mod receiver_restart;
mod same_carrier;
mod sender_restart;
mod srt_server_restart;
mod steady;
mod too_many_links;
mod two_senders;
mod uplink_only_drop;
mod varying_bandwidth;
mod wire_format;
