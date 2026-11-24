//! The recording upload's settings, from the environment (`RecordingSettings::from_environment`).

use std::collections::HashMap;
use std::path::PathBuf;

use bonding_sender::freewheel::{
    BASE_RTT_WINDOW_S_VARIABLE, CHUNK_BYTES_VARIABLE, DEFAULT_CHUNK_BYTES, DRAIN_S_VARIABLE,
    END_IDLE_MS_VARIABLE, IDLE_LIVE_BPS_VARIABLE, IN_FLIGHT_CHUNKS_VARIABLE,
    LIVE_RATIO_PERCENT_VARIABLE, LOSS_TIMEOUT_MS_VARIABLE, QUEUE_TARGET_MS_VARIABLE,
    RECORDING_VARIABLE, RecordingSettings, SHARE_PERCENT_VARIABLE, WINDOW_BYTES_VARIABLE,
    WINDOW_GAIN_CHUNKS_VARIABLE,
};

fn read(variables: &[(&str, &str)]) -> Result<Option<RecordingSettings>, String> {
    let variables: HashMap<String, String> = variables
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    RecordingSettings::from_environment(|name| variables.get(name).cloned())
        .map_err(|error| error.to_string())
}

#[test]
fn no_recording_variable_is_off() {
    assert_eq!(read(&[]), Ok(None));
    assert_eq!(read(&[(RECORDING_VARIABLE, "  ")]), Ok(None));
    assert_eq!(
        read(&[(SHARE_PERCENT_VARIABLE, "30")]),
        Ok(None),
        "the other settings alone switch nothing on"
    );
}

#[test]
fn a_recording_alone_takes_every_default() {
    assert_eq!(
        read(&[(RECORDING_VARIABLE, "/tmp/recording.bin")]),
        Ok(Some(RecordingSettings::new(PathBuf::from(
            "/tmp/recording.bin"
        ))))
    );
    assert_eq!(DEFAULT_CHUNK_BYTES, 1_471, "a full datagram");
}

#[test]
fn every_number_is_a_setting() {
    let settings = read(&[
        (RECORDING_VARIABLE, "/tmp/recording.bin"),
        (CHUNK_BYTES_VARIABLE, "1000"),
        (SHARE_PERCENT_VARIABLE, "75"),
        (LIVE_RATIO_PERCENT_VARIABLE, "250"),
        (QUEUE_TARGET_MS_VARIABLE, "30"),
        (BASE_RTT_WINDOW_S_VARIABLE, "5"),
        (WINDOW_GAIN_CHUNKS_VARIABLE, "2"),
        (IDLE_LIVE_BPS_VARIABLE, "50000"),
        (LOSS_TIMEOUT_MS_VARIABLE, "900"),
        (END_IDLE_MS_VARIABLE, "500"),
        (DRAIN_S_VARIABLE, "120"),
        (WINDOW_BYTES_VARIABLE, "1000000"),
        (IN_FLIGHT_CHUNKS_VARIABLE, "64"),
    ]);
    assert_eq!(
        settings,
        Ok(Some(RecordingSettings {
            path: PathBuf::from("/tmp/recording.bin"),
            chunk_bytes: 1_000,
            share_percent: 75,
            live_ratio_percent: 250,
            queue_target_us: 30_000,
            base_rtt_window_us: 5_000_000,
            window_gain_chunks: 2,
            idle_live_bps: 50_000,
            loss_timeout_us: 900_000,
            end_idle_us: 500_000,
            drain_us: 120_000_000,
            window_bytes: 1_000_000,
            in_flight_chunks: 64,
        }))
    );
}

#[test]
fn a_setting_out_of_range_is_refused_with_its_variable() {
    for (variable, value) in [
        (CHUNK_BYTES_VARIABLE, "0"),
        (CHUNK_BYTES_VARIABLE, "1472"),
        (SHARE_PERCENT_VARIABLE, "0"),
        (SHARE_PERCENT_VARIABLE, "101"),
        (SHARE_PERCENT_VARIABLE, "half"),
        (LIVE_RATIO_PERCENT_VARIABLE, "-1"),
        (LOSS_TIMEOUT_MS_VARIABLE, "0"),
        (QUEUE_TARGET_MS_VARIABLE, "0"),
        (BASE_RTT_WINDOW_S_VARIABLE, "0"),
        (WINDOW_GAIN_CHUNKS_VARIABLE, "0"),
        (IDLE_LIVE_BPS_VARIABLE, "lots"),
        (END_IDLE_MS_VARIABLE, "-1"),
        (DRAIN_S_VARIABLE, "1.5"),
        (WINDOW_BYTES_VARIABLE, "1000"),
        (IN_FLIGHT_CHUNKS_VARIABLE, "0"),
    ] {
        let error = read(&[(RECORDING_VARIABLE, "/tmp/r"), (variable, value)])
            .err()
            .unwrap_or_default();
        assert!(
            error.starts_with(&format!("{variable}={value}: expected ")),
            "{error}"
        );
    }
}

#[test]
fn a_live_ratio_of_zero_holds_the_upload_for_after_the_live() {
    let settings = read(&[
        (RECORDING_VARIABLE, "/tmp/r"),
        (LIVE_RATIO_PERCENT_VARIABLE, "0"),
    ]);
    assert_eq!(
        settings
            .ok()
            .flatten()
            .map(|settings| settings.live_ratio_percent),
        Some(0)
    );
}
