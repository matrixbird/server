use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand_core::OsRng;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use base64::prelude::*;

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct Keys {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl Keys {
    pub fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let public_key_path = format!("{}.private.key", config.matrix.server_name);
        let private_key_path = format!("{}.public.key", config.matrix.server_name);

        if !std::path::Path::new(&public_key_path).exists() || 
            !std::path::Path::new(&private_key_path).exists() {
            generate_keys(&public_key_path, &private_key_path)
                .expect("Could not generate keypair.");
        }

        let keys = read_keys(&public_key_path, &private_key_path);

        match keys {
            Ok(keys) => Ok(keys),
            Err(e) => {
                anyhow::bail!("Could not read keys: {}", e);
            }
        }

    }
}

pub fn generate_keys(public_key_path: &str, private_key_path: &str) -> Result<(), anyhow::Error> {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let private_key_base64 = BASE64_STANDARD.encode(signing_key.to_bytes());

    let public_key_base64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let mut private_file = File::create(private_key_path)?;
    private_file.write_all(private_key_base64.as_bytes())?;

    let mut public_file = File::create(public_key_path)?;
    public_file.write_all(public_key_base64.as_bytes())?;

    tracing::info!("Keypair generated and saved!");
    tracing::info!("Private key stored as Base64 in '{}'", private_key_path);
    tracing::info!("Public key stored as Hex in '{}'", public_key_path);

    Ok(())
}

pub fn read_keys(public_key_path: &str, private_key_path: &str) -> Result<Keys, anyhow::Error> {

    let mut private_key_base64 = String::new();

    OpenOptions::new()
        .read(true)
        .open(private_key_path)?
        .read_to_string(&mut private_key_base64)
        .expect("Could not read private key.");

    let private_key_bytes = BASE64_STANDARD.decode(private_key_base64.trim())?;
    let signing_key = SigningKey::from_bytes(&private_key_bytes.try_into()
        .unwrap());

    let mut public_key_base64 = String::new();

    OpenOptions::new()
        .read(true)
        .open(public_key_path)?
        .read_to_string(&mut public_key_base64)
        .expect("Could not read public key.");

    let public_key_bytes = BASE64_STANDARD.decode(public_key_base64.trim())?;

    let verifying_key = VerifyingKey::from_bytes(&public_key_bytes.try_into()
        .unwrap())?;

    Ok(Keys {
        signing_key,
        verifying_key,
    })
}

pub fn sign_message(signing_key: &SigningKey, message: &[u8]) -> Signature {
    signing_key.sign(message)
}

pub fn verify_signature(verifying_key: &VerifyingKey, message: &[u8], signature: &Signature) -> bool {
    verifying_key.verify(message, signature).is_ok()
}


