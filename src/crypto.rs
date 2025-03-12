use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand_core::OsRng;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use base64::prelude::*;

const PRIVATE_KEY_PATH: &str = "private.key";
const PUBLIC_KEY_PATH: &str = "public.key";

#[derive(Clone, Debug)]
pub struct Keys {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
}

impl Keys {
    pub fn new() -> Result<Self, anyhow::Error> {

    if !std::path::Path::new(PRIVATE_KEY_PATH).exists() || 
        !std::path::Path::new(PUBLIC_KEY_PATH).exists() {
        generate_keys()
            .expect("Could not generate keypair.");
    }

    let keys = read_keys();

        match keys {
            Ok(keys) => Ok(keys),
            Err(e) => {
                anyhow::bail!("Could not read keys: {}", e);
            }
        }

    }
}

pub fn generate_keys() -> Result<(), anyhow::Error> {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let private_key_base64 = BASE64_STANDARD.encode(signing_key.to_bytes());

    let public_key_base64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let mut private_file = File::create(PRIVATE_KEY_PATH)?;
    private_file.write_all(private_key_base64.as_bytes())?;

    let mut public_file = File::create(PUBLIC_KEY_PATH)?;
    public_file.write_all(public_key_base64.as_bytes())?;

    tracing::info!("Keypair generated and saved!");
    tracing::info!("Private key stored as Base64 in '{}'", PRIVATE_KEY_PATH);
    tracing::info!("Public key stored as Hex in '{}'", PUBLIC_KEY_PATH);

    Ok(())
}

pub fn read_keys() -> Result<Keys, anyhow::Error> {

    let mut private_key_base64 = String::new();

    OpenOptions::new()
        .read(true)
        .open(PRIVATE_KEY_PATH)?
        .read_to_string(&mut private_key_base64)
        .expect("Could not read private key.");

    let private_key_bytes = BASE64_STANDARD.decode(private_key_base64.trim())?;
    let signing_key = SigningKey::from_bytes(&private_key_bytes.try_into()
        .unwrap());

    let mut public_key_base64 = String::new();

    OpenOptions::new()
        .read(true)
        .open(PUBLIC_KEY_PATH)?
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


