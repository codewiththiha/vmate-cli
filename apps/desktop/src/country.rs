//! Present relay-country metadata without claiming verified tunnel egress.

use vmate_engine::model::Snapshot;

pub(super) fn name(code: &str) -> Option<&'static str> {
    isocountry::CountryCode::for_alpha2_caseless(code.trim())
        .ok()
        .map(|code| code.name())
}

pub(super) fn status(snapshot: &Snapshot) -> String {
    if snapshot.connected {
        match name(&snapshot.connected_country) {
            Some(country) => format!("Connected · {country}"),
            None => "Connected".into(),
        }
    } else {
        snapshot.status.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_countries_use_names_and_unknowns_stay_generic() {
        for (code, expected) in [
            ("JP", "Japan"),
            ("DE", "Germany"),
            ("FR", "France"),
            ("CA", "Canada"),
            ("br", "Brazil"),
        ] {
            assert_eq!(name(code), Some(expected));
        }
        for code in ["", "UNKNOWN", "ZZ", "not a country"] {
            assert!(name(code).is_none());
        }
        let mut snapshot = Snapshot {
            connected: true,
            connected_country: "JP".into(),
            ..Snapshot::default()
        };
        assert_eq!(status(&snapshot), "Connected · Japan");
        snapshot.connected_country = "UNKNOWN".into();
        assert_eq!(status(&snapshot), "Connected");
        snapshot.connected = false;
        snapshot.connected_country = "JP".into();
        assert_eq!(status(&snapshot), "Not connected");
    }

    #[test]
    fn every_iso_country_has_a_nonempty_display_name() {
        for code in isocountry::CountryCode::as_array() {
            assert!(!name(code.alpha2()).unwrap().is_empty());
        }
    }
}
