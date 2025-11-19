pub mod client_rate_limit;
pub mod http;
pub mod nats;
pub mod rate_limit;

use std::{num::NonZeroU32, time::Duration};

use crate::error::{Error, ModelResult};
use error_stack::report;
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, Copy)]
pub enum RateLimitWindow {
    PerSecond(NonZeroU32),
    PerMinute(NonZeroU32),
    Custom { period: Duration },
}

impl RateLimitWindow {
    /// - `<n>s` → PerSecond(n)
    /// - `<n>m` → PerMinute(n)
    /// - `<n>h` → Custom { period = Duration::from_secs(n * 3600) }
    /// - `<n>d` → Custom { period = Duration::from_secs(n * 86400) }
    pub fn from_string(s: &str) -> Option<Self> {
        if s.is_empty() {
            return None;
        }

        let (num_str, unit) = s.split_at(s.len() - 1);
        let number: u32 = match num_str.parse() {
            Ok(n) if n > 0 => n,
            _ => return None,
        };
        let nonzero = match NonZeroU32::new(number) {
            Some(nz) => nz,
            None => return None,
        };

        match unit {
            "s" => Some(RateLimitWindow::PerSecond(nonzero)),
            "m" => Some(RateLimitWindow::PerMinute(nonzero)),
            "h" => {
                let secs = number as u64 * 3600;
                Some(RateLimitWindow::Custom {
                    period: Duration::from_secs(secs),
                })
            }
            "d" => {
                let secs = number as u64 * 86400;
                Some(RateLimitWindow::Custom {
                    period: Duration::from_secs(secs),
                })
            }
            _ => None,
        }
    }
}

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
        return Err(report!(Error::SerdeDeserialize(
            "Invalid JSON: unterminated string literal".to_string()
        )));
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
