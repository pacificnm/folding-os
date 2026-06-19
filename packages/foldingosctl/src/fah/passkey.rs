use regex::Regex;
use std::sync::LazyLock;

/// Folding@home passkeys are opaque client credentials. v8 commonly uses
/// ~43-character base64-style strings, not fixed 32-hex as older docs describe.
pub const FAH_PASSKEY_MIN_LEN: usize = 8;
pub const FAH_PASSKEY_MAX_LEN: usize = 128;

static FAH_PASSKEY_XML_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)(?:passkey|account-token)[^>]*\bv\s*=\s*["']([^"']+)["']"#)
        .expect("passkey xml pattern compiles")
});

pub fn is_valid_fah_passkey(value: &str) -> bool {
    !value.is_empty()
        && value.len() >= FAH_PASSKEY_MIN_LEN
        && value.len() <= FAH_PASSKEY_MAX_LEN
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

pub fn normalize_passkey_input(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    if let Some(captures) = FAH_PASSKEY_XML_PATTERN.captures(trimmed) {
        let value = captures[1].trim();
        if is_valid_fah_passkey(value) {
            return Ok(value.to_string());
        }
        return Err(passkey_format_error(value.len()));
    }

    if is_valid_fah_passkey(trimmed) {
        return Ok(trimmed.to_string());
    }

    Err(passkey_format_error(trimmed.len()))
}

pub fn passkey_format_error(length: usize) -> String {
    format!(
        "passkey must be {FAH_PASSKEY_MIN_LEN} through {FAH_PASSKEY_MAX_LEN} letters, digits, or base64 characters (+/=); got {length} characters"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_legacy_32_hex_passkey() {
        let key = "0123456789abcdef0123456789abcdef";
        assert_eq!(normalize_passkey_input(key).expect("hex key"), key);
    }

    #[test]
    fn normalize_accepts_v8_style_passkey() {
        let key = "VSEPdVSEhijz2hijn2mVZn2mipUP9ipU1qyQH1qxkZ8";
        assert_eq!(normalize_passkey_input(key).expect("v8 key"), key);
        assert_eq!(key.len(), 43);
    }

    #[test]
    fn normalize_extracts_from_config_xml_line() {
        let key = "VSEPdVSEhijz2hijn2mVZn2mipUP9ipU1qyQH1qxkZ8";
        let xml = format!(r#"<passkey v="{key}"/>"#);
        assert_eq!(normalize_passkey_input(&xml).expect("xml line"), key);
    }

    #[test]
    fn normalize_extracts_from_v8_account_token_xml() {
        let key = "VSEPdVSEhijz2hijn2mVZn2mipUP9ipU1qyQH1qxkZ8";
        let xml = format!(r#"<account-token v="{key}"/>"#);
        assert_eq!(normalize_passkey_input(&xml).expect("v8 xml"), key);
    }

    #[test]
    fn normalize_rejects_too_short() {
        assert!(normalize_passkey_input("1234567").is_err());
    }
}
