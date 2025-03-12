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

    pub fn sign_message(&self, message: &str) -> String {
        let signature: Signature = self.signing_key.sign(message.as_bytes());
        BASE64_STANDARD.encode(signature.to_bytes())
    }

    pub fn verify_signature(&self, verifying_key: &str, message: &str, signature: &str) -> Result<bool, anyhow::Error> {


        let verifying_key_bytes = BASE64_STANDARD.decode(verifying_key)
            .map_err(|e| anyhow::anyhow!("Could not decode public key. {}", e))?;
        let bon = <[u8; 32]>::try_from(verifying_key_bytes)
            .map_err(|_| anyhow::anyhow!("Could not convert public key bytes."))?;

        let verifying_key = VerifyingKey::from_bytes(&bon)
            .map_err(|_| anyhow::anyhow!("Could not create verifying key."))?;


        let signature_bytes = BASE64_STANDARD.decode(signature.trim())?;
        let con = <[u8; 64]>::try_from(signature_bytes)
            .map_err(|_| anyhow::anyhow!("Could not convert signature bytes."))?;
        let signature = Signature::from_bytes(&con);
        let ok = verifying_key.verify(message.as_bytes(), &signature).is_ok();
        Ok(ok)
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
    tracing::info!("Private key stored '{}'", private_key_path);
    tracing::info!("Public key stored '{}'", public_key_path);

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

