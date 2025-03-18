use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand_core::OsRng;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use base64::prelude::*;

use axum::{
    extract::State,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::config::Config;
use crate::AppState;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct Keys {
    pub signing_key: SigningKey,
    pub verifying_key: VerifyingKey,
    pub verify_key_base64: String,
}

impl Keys {
    pub fn new(config: &Config) -> Result<Self, anyhow::Error> {

        let signing_key_path = format!("{}.signing.key", config.matrix.server_name);
        //let verify_key_path = format!("{}.verify.key", config.matrix.server_name);

        if !std::path::Path::new(&signing_key_path).exists() {
            generate_keys(&signing_key_path)
                .expect("Could not generate signing key.");
        }

        let keys = read_keys(&signing_key_path);

        match keys {
            Ok(keys) => Ok(keys),
            Err(e) => {
                anyhow::bail!("Could not read signing key: {}", e);
            }
        }
    }

    pub fn sign_message(&self, message: &str) -> String {
        let signature: Signature = self.signing_key.sign(message.as_bytes());
        BASE64_STANDARD.encode(signature.to_bytes())
    }

    pub fn verify_signature(&self, verifying_key: &str, message: &str, signature: &str) -> Result<bool, anyhow::Error> {

        let verifying_key_bytes = BASE64_STANDARD.decode(verifying_key.trim())
            .map_err(|e| anyhow::anyhow!("Could not decode signing key. {}", e))?;
        let bon = <[u8; 32]>::try_from(verifying_key_bytes)
            .map_err(|_| anyhow::anyhow!("Could not convert signing key bytes."))?;

        let verifying_key = VerifyingKey::from_bytes(&bon)
            .map_err(|e| anyhow::anyhow!("Could not create verifying key. {}", e))?;


        let signature_bytes = BASE64_STANDARD.decode(signature.trim())
            .map_err(|e| anyhow::anyhow!("Could not decode signature. {}", e))?;
        let con = <[u8; 64]>::try_from(signature_bytes)
            .map_err(|_| anyhow::anyhow!("Could not convert signature bytes."))?;
        let signature = Signature::from_bytes(&con);
        let ok = verifying_key.verify(message.as_bytes(), &signature).is_ok();
        Ok(ok)
    }

}

pub fn generate_keys(signing_key_path: &str) -> Result<(), anyhow::Error> {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    //let verifying_key = signing_key.verifying_key();

    let signing_key_base64 = BASE64_STANDARD.encode(signing_key.to_bytes());

    //let verify_key_base64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    let mut signing_key_file = File::create(signing_key_path)?;
    signing_key_file.write_all(signing_key_base64.as_bytes())?;

    //let mut verify_key_file = File::create(verify_key_path)?;
    //verify_key_file.write_all(verify_key_base64.as_bytes())?;

    tracing::info!("Keypair generated and saved.");
    tracing::info!("Signing key stored '{}'", signing_key_path);
    //tracing::info!("Verify key stored '{}'", verify_key_path);

    Ok(())
}

pub fn read_keys(signing_key_path: &str) -> Result<Keys, anyhow::Error> {

    let mut signing_key_base64 = String::new();

    OpenOptions::new()
        .read(true)
        .open(signing_key_path)?
        .read_to_string(&mut signing_key_base64)
        .expect("Could not read signing key.");

    let signing_key_bytes = BASE64_STANDARD.decode(signing_key_base64.trim())?;
    let signing_key = SigningKey::from_bytes(&signing_key_bytes.try_into()
        .unwrap());

    let verifying_key = signing_key.verifying_key();

    let verify_key_base64 = BASE64_STANDARD.encode(verifying_key.to_bytes());

    Ok(Keys {
        signing_key,
        verifying_key,
        verify_key_base64,
    })
}

pub async fn verify_key(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ()> {

    Ok(Json(json!({
        "homeserver": state.config.matrix.server_name,
        "verify_key": state.keys.verify_key_base64,
    })))
}
