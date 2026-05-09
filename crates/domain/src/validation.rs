use crate::errors::DomainError;
use crate::models::location::LocationType;

pub fn validate_username(username: &str) -> Result<(), DomainError> {
    if username.len() < 3 {
        return Err(DomainError::Validation("Username must be at least 3 characters".into()));
    }
    if username.len() > 30 {
        return Err(DomainError::Validation("Username must be at most 30 characters".into()));
    }
    if !username.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
        return Err(DomainError::Validation(
            "Username may only contain alphanumeric characters, underscores, and hyphens".into(),
        ));
    }
    Ok(())
}

pub fn validate_email(email: &str) -> Result<(), DomainError> {
    if !email.contains('@') || !email.contains('.') {
        return Err(DomainError::Validation("Invalid email format".into()));
    }
    if email.len() > 254 {
        return Err(DomainError::Validation("Email must be at most 254 characters".into()));
    }
    Ok(())
}

pub fn validate_score(score: i32) -> Result<(), DomainError> {
    if !(0..=10).contains(&score) {
        return Err(DomainError::Validation("Score must be between 0 and 10".into()));
    }
    Ok(())
}

pub fn validate_abv(abv: f64) -> Result<(), DomainError> {
    if !(0.0..=100.0).contains(&abv) {
        return Err(DomainError::Validation("ABV must be between 0.0 and 100.0".into()));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), DomainError> {
    if password.len() < 8 {
        return Err(DomainError::Validation("Password must be at least 8 characters".into()));
    }
    if password.len() > 128 {
        return Err(DomainError::Validation("Password must be at most 128 characters".into()));
    }
    Ok(())
}

pub fn validate_location_name(name: &str) -> Result<(), DomainError> {
    if name.is_empty() {
        return Err(DomainError::Validation("Location name is required".into()));
    }
    if name.len() > 200 {
        return Err(DomainError::Validation("Location name must be at most 200 characters".into()));
    }
    Ok(())
}

pub fn validate_location_metadata(
    location_type: LocationType,
    metadata: &serde_json::Value,
) -> Result<(), DomainError> {
    let obj = metadata
        .as_object()
        .ok_or_else(|| DomainError::Validation("metadata must be a JSON object".into()))?;

    // Check for unknown keys per location type
    let allowed_keys: &[&str] = match location_type {
        LocationType::Bar | LocationType::Restaurant | LocationType::BreweryTaproom => {
            &["address", "city", "country", "website"]
        }
        LocationType::Festival => {
            &["address", "city", "country", "start_date", "end_date", "website", "organizer"]
        }
        LocationType::Home => &["label"],
        LocationType::Online => &["platform", "url"],
        LocationType::Other => &["description"],
    };

    for key in obj.keys() {
        if !allowed_keys.contains(&key.as_str()) {
            return Err(DomainError::Validation(format!(
                "Unknown metadata field '{}' for location type '{:?}'",
                key, location_type
            )));
        }
    }

    // All metadata values must be strings (or null)
    for (key, value) in obj {
        if !value.is_string() && !value.is_null() {
            return Err(DomainError::Validation(format!(
                "Metadata field '{}' must be a string or null",
                key
            )));
        }
    }

    Ok(())
}

pub fn validate_session_name(name: &str) -> Result<(), DomainError> {
    if name.is_empty() {
        return Err(DomainError::Validation("Session name is required".into()));
    }
    if name.len() > 200 {
        return Err(DomainError::Validation("Session name must be at most 200 characters".into()));
    }
    Ok(())
}

pub fn validate_auto_end_minutes(minutes: i32) -> Result<(), DomainError> {
    if minutes < 30 {
        return Err(DomainError::Validation("Auto-end duration must be at least 30 minutes".into()));
    }
    if minutes > 1440 {
        return Err(DomainError::Validation("Auto-end duration must be at most 24 hours".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_username() {
        assert!(validate_username("beer_lover").is_ok());
        assert!(validate_username("hop-head42").is_ok());
    }

    #[test]
    fn test_invalid_username() {
        assert!(validate_username("ab").is_err());
        assert!(validate_username("no spaces").is_err());
        assert!(validate_username(&"a".repeat(31)).is_err());
    }

    #[test]
    fn test_valid_score() {
        assert!(validate_score(0).is_ok());
        assert!(validate_score(10).is_ok());
        assert!(validate_score(5).is_ok());
    }

    #[test]
    fn test_invalid_score() {
        assert!(validate_score(-1).is_err());
        assert!(validate_score(11).is_err());
    }

    #[test]
    fn test_valid_abv() {
        assert!(validate_abv(0.0).is_ok());
        assert!(validate_abv(5.5).is_ok());
        assert!(validate_abv(100.0).is_ok());
    }

    #[test]
    fn test_invalid_abv() {
        assert!(validate_abv(-0.1).is_err());
        assert!(validate_abv(100.1).is_err());
    }

    #[test]
    fn test_valid_password() {
        assert!(validate_password("securepass").is_ok());
    }

    #[test]
    fn test_invalid_password() {
        assert!(validate_password("short").is_err());
        assert!(validate_password(&"a".repeat(129)).is_err());
    }

    #[test]
    fn test_validate_location_name() {
        assert!(validate_location_name("My Bar").is_ok());
        assert!(validate_location_name("").is_err());
        assert!(validate_location_name(&"a".repeat(201)).is_err());
    }

    #[test]
    fn test_validate_location_metadata_bar() {
        use crate::models::location::LocationType;
        let valid = serde_json::json!({"address": "123 Main St", "city": "Oslo"});
        assert!(validate_location_metadata(LocationType::Bar, &valid).is_ok());

        let invalid_key = serde_json::json!({"start_date": "2025-01-01"});
        assert!(validate_location_metadata(LocationType::Bar, &invalid_key).is_err());

        let invalid_type = serde_json::json!({"address": 123});
        assert!(validate_location_metadata(LocationType::Bar, &invalid_type).is_err());

        let not_obj = serde_json::json!("string");
        assert!(validate_location_metadata(LocationType::Bar, &not_obj).is_err());
    }

    #[test]
    fn test_validate_location_metadata_festival() {
        use crate::models::location::LocationType;
        let valid = serde_json::json!({
            "address": "Convention Center",
            "city": "Berlin",
            "start_date": "2025-06-01",
            "end_date": "2025-06-03"
        });
        assert!(validate_location_metadata(LocationType::Festival, &valid).is_ok());
    }

    #[test]
    fn test_validate_location_metadata_home() {
        use crate::models::location::LocationType;
        let valid = serde_json::json!({"label": "My place"});
        assert!(validate_location_metadata(LocationType::Home, &valid).is_ok());

        let empty = serde_json::json!({});
        assert!(validate_location_metadata(LocationType::Home, &empty).is_ok());
    }

    #[test]
    fn test_validate_location_metadata_online() {
        use crate::models::location::LocationType;
        let valid = serde_json::json!({"platform": "Discord", "url": "https://discord.gg/xyz"});
        assert!(validate_location_metadata(LocationType::Online, &valid).is_ok());
    }

    #[test]
    fn test_validate_session_name() {
        assert!(validate_session_name("Friday Tasting").is_ok());
        assert!(validate_session_name("").is_err());
        assert!(validate_session_name(&"x".repeat(201)).is_err());
    }

    #[test]
    fn test_validate_auto_end_minutes() {
        assert!(validate_auto_end_minutes(180).is_ok());
        assert!(validate_auto_end_minutes(30).is_ok());
        assert!(validate_auto_end_minutes(1440).is_ok());
        assert!(validate_auto_end_minutes(29).is_err());
        assert!(validate_auto_end_minutes(1441).is_err());
    }
}
