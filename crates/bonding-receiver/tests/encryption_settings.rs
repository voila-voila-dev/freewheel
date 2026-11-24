//! The receiver's encryption settings, from the environment.

use std::collections::HashMap;
use std::path::PathBuf;

use bonding_receiver::freewheel::{
    AUTHORIZED_KEYS_VARIABLE, CIPHER_VARIABLE, EncryptionSettings, PLAINTEXT_VARIABLE,
    PRIVATE_KEY_VARIABLE, REPLAY_WINDOW_PACKETS_VARIABLE,
};
use freewheel_noise::{Cipher, DEFAULT_REPLAY_WINDOW_PACKETS};

fn read(pairs: &[(&str, &str)]) -> Result<Option<EncryptionSettings>, String> {
    let environment: HashMap<String, String> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    EncryptionSettings::from_environment(|name| environment.get(name).cloned())
        .map_err(|error| error.to_string())
}

const KEYS: [(&str, &str); 2] = [
    (PRIVATE_KEY_VARIABLE, "/srv/server.key"),
    (AUTHORIZED_KEYS_VARIABLE, "/srv/boxes"),
];

#[test]
fn encrypted_by_default_so_keys_are_required() {
    let error = read(&[]).err().unwrap_or_default();
    assert!(error.contains(PRIVATE_KEY_VARIABLE), "{error}");
    let error = read(&KEYS[..1]).err().unwrap_or_default();
    assert!(error.contains(AUTHORIZED_KEYS_VARIABLE), "{error}");
    assert_eq!(read(&[(PLAINTEXT_VARIABLE, "1")]), Ok(None));
    assert!(read(&[(PLAINTEXT_VARIABLE, "true")]).is_err());
}

#[test]
fn defaults_and_settings() {
    assert_eq!(
        read(&KEYS),
        Ok(Some(EncryptionSettings {
            private_key_path: PathBuf::from("/srv/server.key"),
            authorized_keys_path: PathBuf::from("/srv/boxes"),
            cipher: Cipher::ChaCha20Poly1305,
            replay_window_packets: DEFAULT_REPLAY_WINDOW_PACKETS,
        }))
    );
    let set = [
        KEYS[0],
        KEYS[1],
        (CIPHER_VARIABLE, "aes256gcm"),
        (REPLAY_WINDOW_PACKETS_VARIABLE, "4096"),
    ];
    let settings = read(&set).ok().flatten();
    assert_eq!(settings.as_ref().map(|s| s.cipher), Some(Cipher::Aes256Gcm));
    assert_eq!(settings.map(|s| s.replay_window_packets), Some(4_096));
    for bad in ["0", "63", "2000000", "many"] {
        let set = [KEYS[0], KEYS[1], (REPLAY_WINDOW_PACKETS_VARIABLE, bad)];
        assert!(read(&set).is_err(), "{bad}");
    }
}
