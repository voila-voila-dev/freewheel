//! `freewheel-keygen [--force] PRIVATE_KEY_PATH`: makes a static key pair for a box or the
//! server. Writes the private key to `PRIVATE_KEY_PATH` (mode 0600), the public key to
//! `PRIVATE_KEY_PATH.pub`, and prints the public key: a box's goes in the server's
//! `FREEWHEEL_AUTHORIZED_KEYS` file, the server's in every box's `FREEWHEEL_SERVER_PUBLIC_KEY`.
//!
//! Refuses to replace an existing key unless `--force`: resetting a box is running it again
//! with `--force`, then swapping its public key in the server's list.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use freewheel_noise::KeyPair;

const USAGE: &str = "usage: freewheel-keygen [--force] PRIVATE_KEY_PATH";
/// Owner read and write only.
const PRIVATE_KEY_MODE: u32 = 0o600;

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(&arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: &[String]) -> Result<(), String> {
    let (force, path) = match arguments {
        [path] if path != "--force" => (false, PathBuf::from(path)),
        [flag, path] if flag == "--force" => (true, PathBuf::from(path)),
        _ => return Err(USAGE.to_owned()),
    };
    let pair = KeyPair::generate().map_err(|_| "no randomness for a key".to_owned())?;
    write_key(&path, &format!("{}\n", pair.private.to_hex()), force)?;
    let mut public_path = path.into_os_string();
    public_path.push(".pub");
    let public_path = PathBuf::from(public_path);
    write_key(&public_path, &format!("{}\n", pair.public), force)?;
    println!("{}", pair.public);
    Ok(())
}

fn write_key(path: &Path, text: &str, force: bool) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(PRIVATE_KEY_MODE);
        let mut file = options
            .open(path)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        // `mode` only applies to a new file: a replaced one is narrowed too.
        file.set_permissions(std::fs::Permissions::from_mode(PRIVATE_KEY_MODE))
            .and_then(|()| file.write_all(text.as_bytes()))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }
    #[cfg(not(unix))]
    {
        options
            .open(path)
            .and_then(|mut file| file.write_all(text.as_bytes()))
            .map_err(|error| format!("cannot write {}: {error}", path.display()))
    }
}
