/// Translate raw WebAuthn JsValue error strings into user-friendly messages.
pub fn friendly_webauthn_error(raw: &str) -> String {
    if raw.contains("NotAllowedError") {
        // The browser always says "timed out or was not allowed" for both cases.
        // A true timeout takes 5+ minutes; a cancel is instant. Since we can't
        // distinguish here, use a message that covers both gracefully.
        "Passkey request was cancelled or timed out. Please try again.".into()
    } else if raw.contains("AbortError") {
        "Passkey request was cancelled.".into()
    } else if raw.contains("SecurityError") {
        "Security error — this domain may not be configured for passkeys.".into()
    } else if raw.contains("InvalidStateError") {
        "A passkey already exists for this account on this device.".into()
    } else if raw.contains("NotSupportedError") {
        "Your browser or device doesn't support the required passkey type.".into()
    } else {
        format!("Passkey error: {raw}")
    }
}
