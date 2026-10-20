use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

pub fn flatten_locale(value: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    flatten("", value, &mut keys);
    keys
}

fn flatten(prefix: &str, value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                let next = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                flatten(&next, child, out);
            }
        }
        Value::String(_) => {
            out.insert(prefix.to_string());
        }
        other => panic!("locale leaf {prefix} is not a string: {other}"),
    }
}

pub fn locale_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app/ui/locales")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::all_error_codes;
    use std::fs;

    #[test]
    fn english_and_spanish_have_the_same_keys_and_cover_errors() {
        let dir = locale_dir();
        let en: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("en.json")).unwrap()).unwrap();
        let es: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("es.json")).unwrap()).unwrap();
        let en_keys = flatten_locale(&en);
        let es_keys = flatten_locale(&es);
        assert_eq!(en_keys, es_keys);
        for code in all_error_codes() {
            let key = format!("errors.{code}");
            assert!(en_keys.contains(&key), "missing {key}");
        }
        assert!(en_keys.contains("warning.body"));
        assert!(en_keys.contains("sessions.reported_by_client"));
    }
}
