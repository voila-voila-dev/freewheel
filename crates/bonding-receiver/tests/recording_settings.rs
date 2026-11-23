//! Where the receiver keeps the recordings, from the environment
//! (`RecordingSettings::from_environment`).

use std::collections::HashMap;
use std::path::PathBuf;

use bonding_receiver::freewheel::{
    ACKNOWLEDGED_RANGES_VARIABLE, DIRECTORY_VARIABLE, MAX_ACKNOWLEDGED_RANGES, RecordingSettings,
    WRITER_QUEUE_CHUNKS_VARIABLE,
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
fn no_directory_is_off() {
    assert_eq!(read(&[]), Ok(None));
    assert_eq!(read(&[(DIRECTORY_VARIABLE, "")]), Ok(None));
    assert_eq!(read(&[(ACKNOWLEDGED_RANGES_VARIABLE, "8")]), Ok(None));
}

#[test]
fn a_directory_alone_takes_every_default() {
    assert_eq!(
        read(&[(DIRECTORY_VARIABLE, "/var/recordings")]),
        Ok(Some(RecordingSettings::new(PathBuf::from(
            "/var/recordings"
        ))))
    );
}

#[test]
fn every_number_is_a_setting() {
    assert_eq!(
        read(&[
            (DIRECTORY_VARIABLE, "/var/recordings"),
            (WRITER_QUEUE_CHUNKS_VARIABLE, "100"),
            (ACKNOWLEDGED_RANGES_VARIABLE, "4"),
        ]),
        Ok(Some(RecordingSettings {
            directory: PathBuf::from("/var/recordings"),
            writer_queue_chunks: 100,
            acknowledged_ranges: 4,
        }))
    );
}

#[test]
fn a_setting_out_of_range_is_refused_with_its_variable() {
    let too_many = (MAX_ACKNOWLEDGED_RANGES + 1).to_string();
    for (variable, value) in [
        (WRITER_QUEUE_CHUNKS_VARIABLE, "0"),
        (WRITER_QUEUE_CHUNKS_VARIABLE, "many"),
        (ACKNOWLEDGED_RANGES_VARIABLE, "0"),
        (ACKNOWLEDGED_RANGES_VARIABLE, too_many.as_str()),
    ] {
        let error = read(&[(DIRECTORY_VARIABLE, "/var/r"), (variable, value)])
            .err()
            .unwrap_or_default();
        assert!(
            error.starts_with(&format!("{variable}={value}: expected ")),
            "{error}"
        );
    }
}
