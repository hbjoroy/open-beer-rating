#[derive(Debug, Clone)]
pub struct WebAuthnConfig {
    /// Relying Party ID — a domain string (e.g. "example.com", "localhost")
    pub rp_id: String,
    /// Relying Party display name
    pub rp_name: String,
    /// Allowed origin(s) — full scheme+host+port (e.g. "https://example.com")
    pub origin: String,
}

impl WebAuthnConfig {
    pub fn new(rp_id: &str, origin: &str) -> Self {
        Self {
            rp_id: rp_id.to_string(),
            rp_name: "Open Tappd".to_string(),
            origin: origin.to_string(),
        }
    }

    pub fn with_rp_name(mut self, name: &str) -> Self {
        self.rp_name = name.to_string();
        self
    }
}
