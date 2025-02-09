use regex::Regex;
use once_cell::sync::Lazy;

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

pub fn get_localpart(email: &str) -> Option<(&str, Option<&str>)> {
    email.split('@').next().map(|local| {
        let mut parts = local.splitn(2, '+');
        let base = parts.next().unwrap_or("");
        let tag = parts.next();
        (base, tag)
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_get_localpart() {
        assert_eq!(get_localpart("user@example.com"), Some(("user", None)));
        assert_eq!(get_localpart("admin@matrix.org"), Some(("admin", None)));
        assert_eq!(get_localpart("nobody"), Some(("nobody", None)));
        assert_eq!(get_localpart("user+tag@example.com"), Some(("user", Some("tag"))));
    }
    
    #[test]
    fn test_email_to_matrix_id() {
        assert_eq!(email_to_matrix_id("user@example.com"), Some("@user:example.com".to_string()));
        assert_eq!(email_to_matrix_id("admin@matrix.org"), Some("@admin:matrix.org".to_string()));
        assert_eq!(email_to_matrix_id("invalidemail"), None);
        assert_eq!(email_to_matrix_id("user+tag@example.com"), Some("@user:example.com".to_string()));
    }
}
