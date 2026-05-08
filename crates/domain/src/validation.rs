use crate::errors::DomainError;

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
}
