use serde_json::{Map, Value};

/// Recursively replace all string values equal to `from` with `to`.
pub fn replace_strings_in_json(mut value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(ref mut s) => {
            if s == from {
                Value::String(to.to_string())
            } else {
                Value::String(s.clone())
            }
        }
        Value::Array(arr) => {
            let new_arr = arr
                .into_iter()
                .map(|v| replace_strings_in_json(v, from, to))
                .collect();
            Value::Array(new_arr)
        }
        Value::Object(obj) => {
            let new_obj: Map<String, Value> = obj
                .into_iter()
                .map(|(k, v)| (k, replace_strings_in_json(v, from, to)))
                .collect();
            Value::Object(new_obj)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace() {
        let json = serde_json::json!({
            "a": "123645",
            "b": ["xxx", "123645", { "c": "123645" }],
            "d": 123645
        });

        let replaced = replace_strings_in_json(json, "123645", "111");

        let expected = serde_json::json!({
            "a": "111",
            "b": ["xxx", "111", { "c": "111" }],
            "d": 123645
        });

        assert_eq!(replaced, expected);
    }
}