//! Room display name validation and URL helpers.
//!
//! Provides sanitization of user-supplied display names and extraction
//! from LiveKit join URLs.

/// Validates and sanitizes a room display name.
///
/// Trims leading/trailing whitespace, collapses internal runs of whitespace
/// to a single space, then checks:
/// - non-empty after trimming
/// - at most 80 Unicode scalar values
/// - no forbidden characters: `< > " ' \` ; & \ / { }`
/// - no ASCII control characters
/// - no Unicode bidirectional override characters (U+202A–U+202E, U+2066–U+2069)
///
/// Returns `Some(sanitized)` on success, `None` on any violation.
pub fn validate_room_display_name(raw: &str) -> Option<String> {
    // Reject control characters before any normalization so that e.g. \n is
    // caught even though split_whitespace would otherwise consume it.
    if raw.chars().any(|c| c.is_control()) {
        return None;
    }
    let trimmed: String = raw.split_whitespace().collect::<Vec<&str>>().join(" ");
    if trimmed.is_empty() || trimmed.chars().count() > 80 {
        return None;
    }
    const FORBIDDEN: &[char] = &['<', '>', '"', '\'', '`', ';', '&', '\\', '/', '{', '}'];
    if trimmed.chars().any(|c| FORBIDDEN.contains(&c)) {
        return None;
    }
    if trimmed
        .chars()
        .any(|c| matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'))
    {
        return None;
    }
    Some(trimmed)
}

/// Extracts and validates the `visio` query parameter from a URL.
///
/// Percent-decodes the value, then passes it through [`validate_room_display_name`].
/// Returns `None` if the parameter is absent, empty, or fails validation.
pub fn extract_room_display_name(url: &str) -> Option<String> {
    let query_start = url.find('?')?;
    let query = &url[query_start + 1..];
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("");
        let value = kv.next().unwrap_or("");
        if key == "visio" {
            let decoded = urlencoding::decode(value).ok()?;
            return validate_room_display_name(&decoded);
        }
    }
    None
}

/// Removes the `visio` query parameter from a URL.
///
/// All other query parameters are preserved. If removing the parameter
/// leaves no query string, the `?` is also removed.
pub fn strip_room_display_name_param(url: &str) -> String {
    let Some(query_start) = url.find('?') else {
        return url.to_string();
    };
    let base = &url[..query_start];
    let query = &url[query_start + 1..];
    let remaining: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("");
            key != "visio"
        })
        .collect();
    if remaining.is_empty() {
        base.to_string()
    } else {
        format!("{}?{}", base, remaining.join("&"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_room_display_name ---

    #[test]
    fn validate_simple_name() {
        assert_eq!(
            validate_room_display_name("Alice"),
            Some("Alice".to_string())
        );
    }

    #[test]
    fn validate_trims_whitespace() {
        assert_eq!(
            validate_room_display_name("  Bob  "),
            Some("Bob".to_string())
        );
    }

    #[test]
    fn validate_collapses_internal_spaces() {
        assert_eq!(
            validate_room_display_name("Alice   Bob"),
            Some("Alice Bob".to_string())
        );
    }

    #[test]
    fn validate_empty_returns_none() {
        assert_eq!(validate_room_display_name(""), None);
    }

    #[test]
    fn validate_whitespace_only_returns_none() {
        assert_eq!(validate_room_display_name("   "), None);
    }

    #[test]
    fn validate_too_long_returns_none() {
        // 81 ASCII chars → None
        let name = "a".repeat(81);
        assert_eq!(validate_room_display_name(&name), None);
    }

    #[test]
    fn validate_max_length_ok() {
        // exactly 80 ASCII chars → Some
        let name = "a".repeat(80);
        assert_eq!(validate_room_display_name(&name), Some(name));
    }

    #[test]
    fn validate_multibyte_80_chars_ok() {
        // 80 × 'é' (U+00E9, 2 UTF-8 bytes each) → Some
        let name: String = std::iter::repeat('é').take(80).collect();
        assert_eq!(validate_room_display_name(&name), Some(name));
    }

    #[test]
    fn validate_multibyte_81_chars_none() {
        // 81 × 'é' → None
        let name: String = std::iter::repeat('é').take(81).collect();
        assert_eq!(validate_room_display_name(&name), None);
    }

    #[test]
    fn validate_rejects_angle_brackets() {
        assert_eq!(validate_room_display_name("<script>"), None);
    }

    #[test]
    fn validate_rejects_all_forbidden_chars() {
        for &ch in &['<', '>', '"', '\'', '`', ';', '&', '\\', '/', '{', '}'] {
            let input = format!("Alice{}Bob", ch);
            assert_eq!(
                validate_room_display_name(&input),
                None,
                "expected None for char {:?}",
                ch
            );
        }
    }

    #[test]
    fn validate_rejects_control_chars() {
        // U+0001 is a control character
        assert_eq!(validate_room_display_name("Alice\x01Bob"), None);
        // newline is also a control char
        assert_eq!(validate_room_display_name("Alice\nBob"), None);
    }

    #[test]
    fn validate_rejects_rtl_override() {
        // U+202E RIGHT-TO-LEFT OVERRIDE
        let input = format!("Alice\u{202E}Bob");
        assert_eq!(validate_room_display_name(&input), None);
        // U+2066 LEFT-TO-RIGHT ISOLATE
        let input2 = format!("Alice\u{2066}Bob");
        assert_eq!(validate_room_display_name(&input2), None);
    }

    #[test]
    fn validate_allows_unicode_letters() {
        // Arabic, CJK, accented Latin — all fine
        assert!(validate_room_display_name("مرحبا").is_some());
        assert!(validate_room_display_name("こんにちは").is_some());
        assert!(validate_room_display_name("Ångström").is_some());
    }

    #[test]
    fn validate_allows_parens_and_hyphens() {
        assert_eq!(
            validate_room_display_name("Alice (Team-A)"),
            Some("Alice (Team-A)".to_string())
        );
    }

    // --- extract_room_display_name ---

    #[test]
    fn extract_present_percent_encoded() {
        let url = "https://meet.example.com/room?visio=Hello%20World&token=abc";
        assert_eq!(
            extract_room_display_name(url),
            Some("Hello World".to_string())
        );
    }

    #[test]
    fn extract_absent_returns_none() {
        let url = "https://meet.example.com/room?token=abc";
        assert_eq!(extract_room_display_name(url), None);
    }

    #[test]
    fn extract_empty_value_returns_none() {
        let url = "https://meet.example.com/room?visio=&token=abc";
        assert_eq!(extract_room_display_name(url), None);
    }

    #[test]
    fn extract_invalid_value_xss_returns_none() {
        let url = "https://meet.example.com/room?visio=%3Cscript%3E";
        assert_eq!(extract_room_display_name(url), None);
    }

    #[test]
    fn extract_with_other_params() {
        let url = "https://meet.example.com/room?token=abc&visio=Bob&theme=dark";
        assert_eq!(extract_room_display_name(url), Some("Bob".to_string()));
    }

    // --- strip_room_display_name_param ---

    #[test]
    fn strip_removes_param() {
        let url = "https://meet.example.com/room?visio=Alice&token=abc";
        assert_eq!(
            strip_room_display_name_param(url),
            "https://meet.example.com/room?token=abc"
        );
    }

    #[test]
    fn strip_preserves_other_params() {
        let url = "https://meet.example.com/room?token=abc&visio=Alice&theme=dark";
        assert_eq!(
            strip_room_display_name_param(url),
            "https://meet.example.com/room?token=abc&theme=dark"
        );
    }

    #[test]
    fn strip_no_param_unchanged() {
        let url = "https://meet.example.com/room?token=abc";
        assert_eq!(strip_room_display_name_param(url), url);
    }

    #[test]
    fn strip_only_param_removes_question_mark() {
        let url = "https://meet.example.com/room?visio=Alice";
        assert_eq!(
            strip_room_display_name_param(url),
            "https://meet.example.com/room"
        );
    }

    #[test]
    fn strip_no_query_string_unchanged() {
        let url = "https://meet.example.com/room";
        assert_eq!(strip_room_display_name_param(url), url);
    }
}
