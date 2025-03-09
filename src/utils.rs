use regex::Regex;
use once_cell::sync::Lazy;

use rand::Rng;
use rand::distr::Alphanumeric;

use bcrypt::hash;

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    hash(password.as_bytes(), 10)
}


pub fn generate_magic_code() -> String {
    let mut rng = rand::rng();
    format!("{:06}", rng.random_range(0..1_000_000))
}

pub fn generate_string(length: usize) -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect::<String>()
        .to_uppercase()
}

pub fn generate_invite_code() -> String {
    let first = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();
    let second = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(5)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();

    format!("{}-{}", first, second)
}

use ruma::{
    RoomId, 
    OwnedRoomId
};

pub fn room_id_valid(room_id: &str, server_name: &str) -> Result<OwnedRoomId, String> {

    match RoomId::parse(room_id) {

        Ok(id) => {

            if !room_id.starts_with('!') {
                return Err("Room ID must start with '!'".to_string());
            }

            let pos = room_id.find(':')
                .ok_or_else(|| "Room ID must contain a ':'".to_string())?;

            let domain = &room_id[pos + 1..];

            if domain.is_empty() {
                return Err("Room ID must have a valid domain part".to_string());
            }

            if domain != server_name {
                return Err(format!("Room ID domain part does not match server_name: {} != {}", domain, server_name));
            }

            Ok(id)
        }

        Err(err) => Err(format!("Failed to parse Room ID: {}", err)),
    }
}

static SLUG_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-zA-Z0-9]+").unwrap());


pub fn slugify(s: &str) -> String {
    SLUG_REGEX.replace_all(s, "-").to_string().to_lowercase()
}

pub fn get_localpart(email: String) -> Option<(String, Option<String>)> {
    email.split('@').next().map(|local| {
        let mut parts = local.splitn(2, '+');
        let base = parts.next().unwrap_or("").to_string();
        let tag = parts.next().map(|t| t.to_string());
        (base, tag)
    })
}

pub fn get_mxid_localpart(mxid: &str) -> Option<&str> {
    if mxid.starts_with('@') {
        if let Some(pos) = mxid.find(':') {
            return Some(&mxid[1..pos]);
        }
    }
    None
}

pub fn email_to_matrix_id(email: &str) -> Option<String> {
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() == 2 {
        let mut local_parts = parts[0].splitn(2, '+');
        let base = local_parts.next().unwrap_or("");
        Some(format!("@{}:{}", base, parts[1]))
    } else {
        None
    }
}

pub fn construct_matrix_id(input: &str, homeserver: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if input.contains('@') {
        let parts: Vec<&str> = input.split('@').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return None;
        }
        Some(format!("@{}:{}", parts[0], parts[1]))
    } else {
        Some(format!("@{}:{}", input, homeserver))
    }
}

pub fn get_email_domain(email: &str) -> Result<&str, anyhow::Error> {
    let parts: Vec<&str> = email.split('@').collect();
    match parts.as_slice() {
        [_, domain] if !domain.is_empty() => Ok(*domain),
        _ => Err(anyhow::Error::msg("Invalid email format")),
    }
}


pub fn get_email_subdomain(email: &str) -> Result<&str, anyhow::Error> {
    let parts: Vec<&str> = email.split('@').collect();
    match parts.as_slice() {
        [_, domain] if !domain.is_empty() => {
            let domain_parts: Vec<&str> = domain.trim().split('.').collect();
            domain_parts.first().ok_or(anyhow::Error::msg("No subdomain found")).copied()
        }
        _ => Err(anyhow::Error::msg("Invalid email format")),
    }
}

pub fn localhost_domain(address: String) -> String {
    if let Some(separator_position) = address.rfind(':') {
        let host_part = &address[..separator_position + 1];
        let port_part = &address[separator_position + 1..];
        
        let mut port_digits: Vec<char> = port_part.chars().collect();
        
        if port_digits.len() >= 2 {
            port_digits[1] = '0';
        }
        
        format!("{}{}", host_part, port_digits.into_iter().collect::<String>())
    } else {
        address
    }
}

pub fn replace_email_domain(email: &str, new_domain: &str) -> String {
    let parts: Vec<&str> = email.split('@').collect();
    
    if parts.len() == 2 {
        format!("{}@{}", parts[0], new_domain)
    } else {
        email.to_string()  
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_email() {
        assert_eq!(get_email_subdomain("user@pm-bounces.matrixbird.com").unwrap(), "pm-bounces");
        assert_eq!(get_email_subdomain("user@pm-bounces.matrixbird.com ").unwrap(), "pm-bounces");
    }

    #[test]
    fn test_invalid_email() {
        assert!(get_email_subdomain("invalid.email").is_err());
        assert!(get_email_subdomain("user@").is_err());
    }

    #[test]
    fn test_get_localpart() {
        assert_eq!(get_localpart("user@example.com".to_string()), Some(("user".to_string(), None)));
        assert_eq!(get_localpart("admin@matrix.org".to_string()), Some(("admin".to_string(), None)));
        assert_eq!(get_localpart("nobody".to_string()), Some(("nobody".to_string(), None)));
        assert_eq!(get_localpart("user+tag@example.com".to_string()), Some(("user".to_string(), Some("tag".to_string()))));
    }

    #[test]
    fn test_email_to_matrix_id() {
        assert_eq!(email_to_matrix_id("user@example.com"), Some("@user:example.com".to_string()));
        assert_eq!(email_to_matrix_id("admin@matrix.org"), Some("@admin:matrix.org".to_string()));
        assert_eq!(email_to_matrix_id("invalidemail"), None);
        assert_eq!(email_to_matrix_id("user+tag@example.com"), Some("@user:example.com".to_string()));
    }
}
