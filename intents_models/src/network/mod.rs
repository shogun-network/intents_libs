pub mod http;
pub mod nats;
use crate::error::{Error, ModelResult};
use error_stack::report;
use serde::de::DeserializeOwned;

fn calculate_json_depth(
    data: &[u8],
    max_json_depth: usize,
    chunk_processing_interval: usize,
) -> ModelResult<usize> {
    let mut current_depth = 0;
    let mut max_depth_seen = 0;
    let mut inside_string = false;
    let mut position = 0;
    let mut i = 0;

    while i < data.len() {
        let byte = data[i];
        position += 1;

        if inside_string {
            match byte {
                b'"' => {
                    // Check if this quote is escaped by counting preceding backslashes
                    let mut escape_count = 0;
                    let mut j = i;
                    while j > 0 {
                        j -= 1;
                        if data[j] == b'\\' {
                            escape_count += 1;
                        } else {
                            break;
                        }
                    }

                    // If even number of backslashes (including 0), quote is not escaped
                    if escape_count % 2 == 0 {
                        inside_string = false;
                    }
                }
                _ => {
                    // Inside string, ignore other characters
                }
            }
        } else {
            match byte {
                b'"' => {
                    inside_string = true;
                }
                b'{' | b'[' => {
                    current_depth += 1;
                    max_depth_seen = max_depth_seen.max(current_depth);

                    // IMMEDIATE rejection if depth exceeded
                    if max_depth_seen > max_json_depth {
                        return Err(report!(Error::SerdeDeserialize(format!(
                            "JSON depth limit exceeded at position {position}: depth {max_depth_seen}, max {max_json_depth}"
                        ))));
                    }
                }
                b'}' | b']' => {
                    if current_depth == 0 {
                        return Err(report!(Error::SerdeDeserialize(format!(
                            "Invalid JSON: unmatched closing bracket at position {position}"
                        ))));
                    }
                    current_depth -= 1;
                }
                _ => {
                    // Ignore whitespace and other characters outside strings
                }
            }
        }

        // Performance safeguard
        if position % chunk_processing_interval == 0 && max_depth_seen > max_json_depth {
            return Err(report!(Error::SerdeDeserialize(format!(
                "JSON processing timeout - malicious payload detected at position {position}"
            ))));
        }

        i += 1;
    }

    // Final validation
    if current_depth != 0 {
        return Err(report!(Error::SerdeDeserialize(format!(
            "Invalid JSON: {current_depth} unmatched opening brackets"
        ))));
    }

    if inside_string {
        return Err(report!(Error::SerdeDeserialize(format!(
            "Invalid JSON: unterminated string literal"
        ))));
    }

    Ok(max_depth_seen)
}

pub fn validate_and_parse_json<T>(
    data: &[u8],
    max_request_body_size: usize,
    max_json_depth: usize,
    chunk_processing_interval: usize,
) -> ModelResult<T>
where
    T: DeserializeOwned,
{
    // Size validation
    if data.len() > max_request_body_size {
        return Err(report!(Error::TooLargeRequestBody(format!(
            "Request too large: {} bytes (max: {})",
            data.len(),
            max_request_body_size
        ))));
    }

    // Depth validation
    let nesting_depth = calculate_json_depth(data, max_json_depth, chunk_processing_interval)?;
    if nesting_depth > max_json_depth {
        return Err(report!(Error::SerdeDeserialize(format!(
            "JSON too deeply nested: {nesting_depth} levels (max: {max_json_depth})"
        ))));
    }

    // Standard parsing - this is all you need
    serde_json::from_slice(data)
        .map_err(|e| report!(Error::SerdeDeserialize(format!("JSON parsing error: {e}"))))
}
