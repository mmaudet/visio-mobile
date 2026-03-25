//! Security-focused unit tests for visio-core.
//!
//! These tests validate input sanitization, XSS prevention, and
//! protection against common attack vectors.
//!
//! Run with: cargo test --test security_tests

use visio_core::{AuthService, ChatService};

// ── AuthService URL parsing security tests ─────────────────────────────

#[test]
fn test_parse_meet_url_rejects_invalid_schemes() {
    // Reject javascript: scheme
    assert!(AuthService::parse_meet_url("javascript:alert(1)").is_err());
    // Reject data: scheme
    assert!(AuthService::parse_meet_url("data:text/html,<script>").is_err());
    // Reject file: scheme
    assert!(AuthService::parse_meet_url("file:///etc/passwd").is_err());
}

#[test]
fn test_parse_meet_url_handles_null_bytes() {
    // Null bytes should be rejected
    let result = AuthService::parse_meet_url("meet.example.com/room\x00name");
    assert!(result.is_err(), "null bytes should be rejected");
}

#[test]
fn test_parse_meet_url_rejects_path_traversal() {
    // Path traversal attempts should not affect the instance
    assert!(AuthService::parse_meet_url("meet.example.com/../evil").is_err());
    assert!(AuthService::parse_meet_url("meet.example.com/room/../../etc/passwd").is_err());
}

#[test]
fn test_extract_slug_rejects_special_chars() {
    // Slugs should only contain lowercase letters and dashes
    assert!(AuthService::extract_slug("room<script>").is_err());
    assert!(AuthService::extract_slug("room<script").is_err());
    assert!(AuthService::extract_slug("room\"onclick").is_err());
    assert!(AuthService::extract_slug("room'onclick").is_err());
}

// ── ChatService XSS prevention tests ────────────────────────────────────

#[test]
fn test_xss_event_handlers_stripped() {
    let tests = vec![
        "img onerror='alert(1)' src=x",
        "div onclick='evil()' clicked",
        "body onload='malicious()'",
        "svg onmouseover='attack()'",
    ];

    for input in tests {
        let result = ChatService::validate_message(input).unwrap();
        let lower = result.to_lowercase();
        assert!(
            !lower.contains("onerror"),
            "onerror not stripped: {}",
            result
        );
        assert!(
            !lower.contains("onclick"),
            "onclick not stripped: {}",
            result
        );
        assert!(!lower.contains("onload"), "onload not stripped: {}", result);
        assert!(
            !lower.contains("onmouseover"),
            "onmouseover not stripped: {}",
            result
        );
    }
}

#[test]
fn test_xss_javascript_protocol_stripped() {
    let tests = vec![
        "javascript:alert(1)",
        "JAVASCRIPT:void(0)",
        "click here javascript:evil() now",
    ];

    for input in tests {
        let result = ChatService::validate_message(input).unwrap();
        assert!(
            !result.to_lowercase().contains("javascript:"),
            "JS protocol not stripped: {}",
            result
        );
    }
}

#[test]
fn test_xss_vbscript_protocol_stripped() {
    let result = ChatService::validate_message("vbscript:msgbox(1)").unwrap();
    assert!(!result.to_lowercase().contains("vbscript:"));
}

#[test]
fn test_xss_data_url_stripped() {
    let result = ChatService::validate_message("data:text/html,<script>alert(1)</script>").unwrap();
    assert!(!result.to_lowercase().contains("data:text/html"));
}

#[test]
fn test_xss_unicode_bypass_attempts() {
    // Test common Unicode bypass techniques for event handlers
    let tests = vec![
        "on\u{0000}click='evil'", // null byte
        "on\u{0009}click='evil'", // tab
        "on\u{000a}click='evil'", // newline
    ];

    for input in tests {
        let result = ChatService::validate_message(input).unwrap();
        // Should either strip or reject
        assert!(
            !result.to_lowercase().contains("onclick"),
            "Unicode bypass succeeded: {}",
            result
        );
    }
}

#[test]
fn test_html_entity_xss_stripped() {
    // Test HTML entity encoded XSS attempts
    let result = ChatService::validate_message("&#115;cript>alert(1)").unwrap();
    // HTML entities should be stripped
    assert!(!result.to_lowercase().contains("script"));
}

#[test]
fn test_message_length_after_sanitization() {
    // Message with HTML tags should still respect length limits
    let long_html = format!("<script>{}</script>", "a".repeat(2001));
    assert!(ChatService::validate_message(&long_html).is_err());
}

#[test]
fn test_valid_messages_not_affected() {
    let valid_messages = vec![
        "Hello, world!",
        "This is a normal message.",
        "Numbers: 12345",
        "Special chars: !@#$%^&*()",
        "Emoji: 😀🎉🔥",
        "URL: <https://example.com>",
    ];

    for msg in valid_messages {
        let result = ChatService::validate_message(msg).unwrap();
        assert_eq!(result, msg, "Message should not be modified: {}", msg);
    }
}

// ── Input validation edge cases ─────────────────────────────────────────

#[test]
fn test_empty_and_whitespace_messages() {
    assert!(ChatService::validate_message("").is_err());
    assert!(ChatService::validate_message("   ").is_err());
    assert!(ChatService::validate_message("\n\t\r").is_err());
}

#[test]
fn test_very_long_input_rejected() {
    let very_long = "a".repeat(10000);
    assert!(ChatService::validate_message(&very_long).is_err());
}

#[test]
fn test_unicode_input_handled() {
    // Unicode should be accepted but sanitized if dangerous
    let result = ChatService::validate_message("Hello 世界！🌍").unwrap();
    assert!(result.contains("世界"));
    assert!(result.contains("🌍"));
}

#[test]
fn test_sql_injection_patterns_not_relevant_but_safe() {
    // While we don't use SQL, ensure dangerous patterns are sanitized
    let result = ChatService::validate_message("'; DROP TABLE users; --").unwrap();
    // Should be accepted as it's just text (no DB interaction)
    assert!(!result.is_empty());
}

// ── Rate limiting and DoS prevention ────────────────────────────────────

#[test]
fn test_repeated_html_parsing_safe() {
    // Ensure repeated parsing completes and strips all script tags
    let input = "<script>x</script>".repeat(100);
    let result = ChatService::validate_message(&input);
    if let Ok(sanitized) = result {
        assert!(!sanitized.to_lowercase().contains("<script"));
    }
    // Err (too long) is also acceptable — the point is no panic or hang
}

#[test]
fn test_regex_rejection_safe() {
    // Test ReDoS protection: long input must be rejected (>2000 chars), not hang
    let input = "a".repeat(10000) + "<script>x</script>";
    assert!(
        ChatService::validate_message(&input).is_err(),
        "input exceeding MAX_MESSAGE_LENGTH should be rejected"
    );
}
