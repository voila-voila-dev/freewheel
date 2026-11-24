//! The sender's encryption settings, from the environment.

use std::collections::HashMap;
use std::path::PathBuf;

use bonding_sender::freewheel::{
    CIPHER_VARIABLE, EncryptionSettings, PLAINTEXT_VARIABLE, PRIVATE_KEY_VARIABLE,
    SERVER_PUBLIC_KEY_VARIABLE,
};
use freewheel_noise::Cipher;

fn read(pairs: &[(&str, &str)]) -> Result<Option<EncryptionSettings>, String> {
    let environment: HashMap<String, String> = pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    EncryptionSettings::from_environment(|name| environment.get(name).cloned())
        .map_err(|error| error.to_string())
}

#[test]
fn encrypted_by_default_so_keys_are_required() {
    let error = read(&[]).err().unwrap_or_default();
    assert!(error.contains(PRIVATE_KEY_VARIABLE), "{error}");
    assert!(error.contains("FREEWHEEL_PLAINTEXT=1"), "{error}");
    let error = read(&[(PRIVATE_KEY_VARIABLE, "/box.key")])
        .err()
        .unwrap_or_default();
    assert!(error.contains(SERVER_PUBLIC_KEY_VARIABLE), "{error}");
}

#[test]
fn plaintext_only_when_asked() {
    assert_eq!(read(&[(PLAINTEXT_VARIABLE, "1")]), Ok(None));
    assert!(read(&[(PLAINTEXT_VARIABLE, "yes")]).is_err());
    assert!(
        read(&[(PLAINTEXT_VARIABLE, "0")]).is_err(),
        "0 is encrypted: keys needed"
    );
}

#[test]
fn paths_and_cipher_are_read() {
    let keys = [
        (PRIVATE_KEY_VARIABLE, " /etc/freewheel/box.key "),
        (SERVER_PUBLIC_KEY_VARIABLE, "/etc/freewheel/server.pub"),
    ];
    assert_eq!(
        read(&keys),
        Ok(Some(EncryptionSettings {
            private_key_path: PathBuf::from("/etc/freewheel/box.key"),
            server_public_key_path: PathBuf::from("/etc/freewheel/server.pub"),
            cipher: Cipher::ChaCha20Poly1305,
        }))
    );
    let with_aes = [keys[0], keys[1], (CIPHER_VARIABLE, "AES256GCM")];
    assert_eq!(
        read(&with_aes)
            .ok()
            .flatten()
            .map(|settings| settings.cipher),
        Some(Cipher::Aes256Gcm)
    );
    let unknown = [keys[0], keys[1], (CIPHER_VARIABLE, "rot13")];
    assert!(read(&unknown).is_err());
}

#[test]
fn missing_key_files_are_reported_by_path() {
    let Ok(Some(settings)) = read(&[
        (PRIVATE_KEY_VARIABLE, "/nonexistent/box.key"),
        (SERVER_PUBLIC_KEY_VARIABLE, "/nonexistent/server.pub"),
    ]) else {
        unreachable!("both set")
    };
    let error = settings.read_keys().err().map(|error| error.to_string());
    assert!(
        error
            .as_deref()
            .is_some_and(|error| error.contains("/nonexistent/box.key")),
        "{error:?}"
    );
}
