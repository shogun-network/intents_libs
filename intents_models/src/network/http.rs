use crate::error::{Error, ModelResult};
use error_stack::{ResultExt, report};
use reqwest::Response;
use serde::de::DeserializeOwned;
use serde_json::value::Value;
use tracing::error;

#[derive(Debug)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
}

impl HttpMethod {
    pub fn to_string(&self) -> &str {
        match self {
            HttpMethod::GET => "GET",
            HttpMethod::POST => "POST",
            HttpMethod::PUT => "PUT",
            HttpMethod::DELETE => "DELETE",
        }
    }
}

/// Converts a JSON Value into a sorted URL query string
///
/// Takes a JSON object and converts it to a URL-encoded query string with parameters
/// sorted alphabetically by key.
///
/// # Arguments
///
/// * `value` - JSON Value object to convert
///
/// # Returns
///
/// Returns `ModelResult<String>` containing:
/// - String: URL-encoded query string with sorted parameters if successful
/// - AggregatorErrors: If input is not a valid JSON object
///
/// # Errors
///
/// Returns `AggregatorErrors::InvalidRequestBody` if:
/// - Input value is not a JSON object
///
pub fn value_to_sorted_querystring(value: &Value) -> ModelResult<String> {
    let mut pairs: Vec<(String, String)> = match value {
        Value::Object(map) => map
            .iter()
            .filter(|(_, v)| !matches!(v, Value::Null))
            .map(|(k, v)| {
                let value_str = match v {
                    Value::String(s) => s.to_string(),
                    _ => v.to_string(),
                };
                (k.clone(), value_str)
            })
            .collect(),
        _ => {
            return Err(report!(Error::ParseError)
                .attach_printable(format!("Invalid JSON Object: {value:?}")));
        }
    };

    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(pairs
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join("&"))
}

pub async fn handle_reqwest_response<T: DeserializeOwned>(response: Response) -> ModelResult<T> {
    let response_code: u16 = response.status().as_u16();
    match response_code {
        0..=399 => {
            // Check the Content-Type header to determine format
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok())
                .unwrap_or("");

            let response_body: T =
            // Use Json as default content type
                if content_type.contains("application/json") || content_type.is_empty() {
                    // Handle JSON response
                    // DEBUG:
                    // let json: Value = response
                    //     .json()
                    //     .await
                    //     .change_context(Error::SerdeDeserialize("Failed to deserialize JSON".to_string()))?;
                    // println!("JSON Response: {}", serde_json::to_string_pretty(&json).unwrap());
                    // serde_json::from_value(json)
                    //     .change_context(Error::SerdeDeserialize("Failed to deserialize JSON".to_string()))?
                    response
                        .json()
                        .await
                        .change_context(Error::SerdeDeserialize("Failed to deserialize JSON".to_string()))?
                } else if content_type.contains("text/") {
                    // Handle text response
                    let text = response
                        .text()
                        .await
                        .change_context(Error::ReqwestError("Failed to get text from response".to_string()))?;

                    // Try to parse text as JSON if T expects it
                    serde_json::from_str(format!("\"{text}\"").as_str())
                        .change_context(Error::SerdeDeserialize("Failed to deserialize text as JSON".to_string()))?
                } else {
                    // Handle other content types
                    return Err(report!(Error::ParseError)
                        .attach_printable(format!("Unsupported Content-Type: {content_type}")));
                };

            Ok(response_body)
        }
        _ => {
            let error_body = response.text().await.change_context(Error::ReqwestError(
                "Failed to get text from response".to_string(),
            ))?;

            error!("Error Body: {}", &error_body);

            Err(report!(Error::ReqwestError(error_body)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_http_method_to_string() {
        assert_eq!(HttpMethod::GET.to_string(), "GET");
        assert_eq!(HttpMethod::POST.to_string(), "POST");
        assert_eq!(HttpMethod::PUT.to_string(), "PUT");
        assert_eq!(HttpMethod::DELETE.to_string(), "DELETE");
    }

    #[test]
    fn test_value_to_sorted_querystring_success() {
        let value = json!({
            "key1": "val1",
            "key4": "val4",
            "key2": "val2",
            "key3": null,
        });

        let result = value_to_sorted_querystring(&value).unwrap();
        assert_eq!(result, "key1=val1&key2=val2&key4=val4");
    }

    #[test]
    fn test_value_to_sorted_querystring_different_types() {
        let value = json!({
            "string_key": "text_value",
            "number_key": 42,
            "boolean_key": true,
            "float_key": 3.5
        });

        let result = value_to_sorted_querystring(&value).unwrap();
        assert_eq!(
            result,
            "boolean_key=true&float_key=3.5&number_key=42&string_key=text_value"
        );
    }

    #[test]
    fn test_value_to_sorted_querystring_empty_object() {
        let value = json!({});
        let result = value_to_sorted_querystring(&value).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_value_to_sorted_querystring_only_nulls() {
        let value = json!({
            "key1": null,
            "key2": null
        });

        let result = value_to_sorted_querystring(&value).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_value_to_sorted_querystring_mixed_with_nulls() {
        let value = json!({
            "valid_key": "value",
            "null_key": null,
            "another_valid": "another"
        });

        let result = value_to_sorted_querystring(&value).unwrap();
        assert_eq!(result, "another_valid=another&valid_key=value");
    }

    #[test]
    fn test_value_to_sorted_querystring_invalid_json_array() {
        let value = json!(["not", "an", "object"]);
        let result = value_to_sorted_querystring(&value);

        assert!(result.is_err());
        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(error_msg.contains("Invalid JSON Object"));
    }

    #[test]
    fn test_value_to_sorted_querystring_invalid_json_string() {
        let value = json!("just a string");
        let result = value_to_sorted_querystring(&value);

        assert!(result.is_err());
    }

    #[test]
    fn test_value_to_sorted_querystring_invalid_json_number() {
        let value = json!(42);
        let result = value_to_sorted_querystring(&value);

        assert!(result.is_err());
    }

    #[test]
    fn test_value_to_sorted_querystring_nested_object() {
        let value = json!({
            "simple": "value",
            "nested": {"inner": "object"}
        });

        let result = value_to_sorted_querystring(&value).unwrap();
        // Nested objects should be stringified
        assert!(
            result.contains("nested={\"inner\":\"object\"}")
                || result.contains("nested=%7B%22inner%22:%22object%22%7D")
        );
        assert!(result.contains("simple=value"));
    }
}
