/// Translate raw WebAuthn JsValue error strings into user-friendly messages.
pub fn friendly_webauthn_error(raw: &str) -> String {
    if raw.contains("NotAllowedError") {
        if raw.contains("timed out") {
            "Passkey request timed out. Please try again.".into()
        } else {
            "Passkey request was cancelled.".into()
        }
    } else if raw.contains("SecurityError") {
        "Security error — this domain may not be configured for passkeys.".into()
    } else if raw.contains("InvalidStateError") {
        "A passkey already exists for this account on this device.".into()
    } else if raw.contains("NotSupportedError") {
        "Your browser or device doesn't support the required passkey type.".into()
    } else if raw.contains("AbortError") {
        "Passkey request was cancelled.".into()
    } else {
        format!("Passkey error: {raw}")
    }
}
