use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Badge {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub icon_url: Option<String>,
    pub criteria_type: String,
    pub criteria_value: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBadge {
    pub user_id: Uuid,
    pub badge_id: Uuid,
    pub earned_at: DateTime<Utc>,
}

/// Recognize explicit saint titles in venue names, rather than guessing from
/// personal names or matching substrings (e.g. "Stout" or "High Street").
fn has_saint_name(name: &str) -> bool {
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphabetic())
        .filter(|word| !word.is_empty())
        .map(str::to_lowercase)
        .collect();
    let words = match words.first().map(String::as_str) {
        Some("the" | "pub" | "bar" | "cafe" | "café") => &words[1..],
        _ => &words[..],
    };
    matches!(
        words.first().map(String::as_str),
        Some(
            "st" | "skt"
                | "sankt"
                | "sankta"
                | "saint"
                | "sainte"
                | "sint"
                | "san"
                | "sant"
                | "santa"
                | "santo"
        )
    ) && words
        .get(1)
        .is_some_and(|word| !matches!(word.as_str(), "pub" | "bar" | "street" | "road" | "hotel"))
}

/// Untappd's flat export has no venue category. Only count an unclassified
/// venue when its name explicitly identifies a pub/bar; never reclassify it.
pub fn is_saint_pub(name: &str, location_type: super::location::LocationType) -> bool {
    use super::location::LocationType;
    has_saint_name(name)
        && (location_type == LocationType::Bar
            || (location_type == LocationType::Other
                && name.split(|c: char| !c.is_alphabetic()).any(|word| {
                    matches!(
                        word.to_lowercase().as_str(),
                        "pub" | "bar" | "tavern" | "inn" | "ølpub"
                    )
                })))
}

#[cfg(test)]
mod tests {
    use super::super::location::LocationType;
    use super::*;

    #[test]
    fn recognizes_explicit_saint_titles_and_punctuation() {
        for name in [
            "St. Patrick’s",
            "The St. Edmund's",
            "Sankt Pauli",
            "Skt. Hans",
            "Sainte-Thérèse",
            "Sint Arnold",
            "San Lorenzo",
            "Santa Maria",
            "  SAINT   OLAV  ",
            "Pub St.Peter",
        ] {
            assert!(is_saint_pub(name, LocationType::Bar), "{name}");
        }
    }

    #[test]
    fn requires_a_pub_and_a_saint_title_not_substrings() {
        for name in [
            "Stout Bar",
            "High Street",
            "High St. Bar",
            "Saints Pub",
            "St.",
            "St. Pub",
            "Patrick's",
            "The Brewery",
        ] {
            assert!(!is_saint_pub(name, LocationType::Bar), "{name}");
        }
        for kind in [
            LocationType::Restaurant,
            LocationType::Festival,
            LocationType::Home,
            LocationType::Online,
            LocationType::BreweryTaproom,
            LocationType::Other,
        ] {
            assert!(!is_saint_pub("St. Patrick's", kind));
        }
        assert!(is_saint_pub("St. Patrick's Pub", LocationType::Other));
        assert!(is_saint_pub("The Saint Arnold Tavern", LocationType::Other));
        assert!(!is_saint_pub("Saint Mary Church", LocationType::Other));
        assert!(!is_saint_pub("St. Patrick's Pub", LocationType::Restaurant));
    }
}
