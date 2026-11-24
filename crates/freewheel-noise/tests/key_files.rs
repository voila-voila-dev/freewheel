//! Key files: what `freewheel-keygen` writes, what the binaries read.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::process::Command;

use freewheel_noise::{AuthorizedKeys, KeyError, KeyPair, KeyProblem, PrivateKey, PublicKey};

fn scratch(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("freewheel-noise-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

#[test]
fn keys_read_back_from_their_hex() {
    let pair = KeyPair::generate().unwrap();
    assert_eq!(
        PrivateKey::from_hex(&pair.private.to_hex()),
        Some(pair.private.clone())
    );
    assert_eq!(
        PublicKey::from_hex(&pair.public.to_string()),
        Some(pair.public)
    );
    assert_eq!(
        PublicKey::from_hex(&pair.public.to_string().to_uppercase()),
        Some(pair.public)
    );
    assert_eq!(PublicKey::from_hex("abc"), None);
    assert_eq!(PublicKey::from_hex(&"g".repeat(64)), None);
}

#[test]
fn a_private_key_is_never_printed() {
    let pair = KeyPair::generate().unwrap();
    let printed = format!("{:?}", pair.private);
    assert!(!printed.contains(&pair.private.to_hex()[..8]), "{printed}");
}

#[test]
fn the_authorised_list_takes_names_and_comments() {
    let first = KeyPair::generate().unwrap().public;
    let second = KeyPair::generate().unwrap().public;
    let unknown = KeyPair::generate().unwrap().public;
    let text = format!("# boxes\n\n{first} box-1\n  {second}\n");
    let list = AuthorizedKeys::parse(&text).unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.contains(&first) && list.contains(&second) && !list.contains(&unknown));
    assert_eq!(
        AuthorizedKeys::parse(&format!("{first}\nnot a key\n")),
        Err(2)
    );
    assert!(AuthorizedKeys::parse("").unwrap().is_empty());
}

#[test]
fn keygen_writes_a_pair_the_binaries_read() {
    let directory = scratch("keygen");
    let path = directory.join("box.key");
    let output = Command::new(env!("CARGO_BIN_EXE_freewheel-keygen"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let printed = String::from_utf8(output.stdout).unwrap();
    let public = PublicKey::from_hex(printed.trim()).unwrap();
    assert_eq!(
        PublicKey::read_file(&directory.join("box.key.pub")),
        Ok(public)
    );
    assert!(PrivateKey::read_file(&path).is_ok());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let again = Command::new(env!("CARGO_BIN_EXE_freewheel-keygen"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(!again.status.success(), "an existing key is kept");
    let reset = Command::new(env!("CARGO_BIN_EXE_freewheel-keygen"))
        .arg("--force")
        .arg(&path)
        .output()
        .unwrap();
    assert!(reset.status.success());
    assert_ne!(
        PublicKey::read_file(&directory.join("box.key.pub")),
        Ok(public),
        "a reset is a new key"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[cfg(unix)]
#[test]
fn a_private_key_others_may_read_is_refused() {
    use std::os::unix::fs::PermissionsExt;
    let directory = scratch("mode");
    let path = directory.join("open.key");
    let pair = KeyPair::generate().unwrap();
    std::fs::write(&path, pair.private.to_hex()).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        PrivateKey::read_file(&path),
        Err(KeyError {
            path: path.clone(),
            problem: KeyProblem::ReadableByOthers { mode: 0o644 },
        })
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_file_with_no_key_or_two_is_refused() {
    let directory = scratch("count");
    let empty = directory.join("empty.pub");
    std::fs::write(&empty, "# nothing\n").unwrap();
    assert_eq!(
        PublicKey::read_file(&empty).map_err(|error| error.problem),
        Err(KeyProblem::Missing)
    );
    let two = directory.join("two.pub");
    let key = KeyPair::generate().unwrap().public;
    std::fs::write(&two, format!("{key}\n{key}\n")).unwrap();
    assert_eq!(
        PublicKey::read_file(&two).map_err(|error| error.problem),
        Err(KeyProblem::Malformed { line: 2 })
    );
    let _ = std::fs::remove_dir_all(&directory);
}
