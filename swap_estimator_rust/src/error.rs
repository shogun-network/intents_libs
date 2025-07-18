use error_stack::{AttachmentKind, FrameKind, Report};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type EstimatorResult<T> = error_stack::Result<T, Error>;

#[derive(Error, Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum Error {
    #[error("Parse error")]
    ParseError,

    #[error("Reqwest error")]
    ReqwestError,

    #[error("Response error")]
    ResponseError,

    #[error("Models error")]
    ModelsError,

    #[error("Unknown error")]
    Unknown,
}

pub trait ReportDisplayExt {
    fn format(&self) -> String;
}

impl ReportDisplayExt for Report<Error> {
    fn format(&self) -> String {
        let mut output = String::new();

        let frames = self.current_frames();

        for frame in frames.into_iter() {
            if let FrameKind::Attachment(AttachmentKind::Printable(attachment)) = frame.kind() {
                output.push_str(&format!(" {} ", attachment));
            }
        }

        output.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use error_stack::report;

    #[test]
    fn test_format_report() {
        let report = report!(Error::ParseError).attach_printable("test1");
        assert_eq!("test1".to_string(), report.format());
    }
}
