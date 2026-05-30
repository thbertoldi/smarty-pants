//! Wire types shared between daemon and CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Paraphrase { mode: String },
    Status,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok { generated_chars: usize, ms: u64 },
    Empty,
    Busy,
    ModelLoading,
    Status { healthy: bool, model_loaded: bool, mode_count: usize },
    Error {
        // Field is `error_kind` because the enum's serde tag is `kind`;
        // a `kind` field here would collide with the tag at the wire layer.
        error_kind: ErrorKind,
        message: String,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Capture,
    Inference,
    Timeout,
    Inject,
    Internal,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_paraphrase_roundtrip() {
        let req = Request::Paraphrase { mode: "rewrite".into() };
        let s = serde_json::to_string(&req).unwrap();
        assert_eq!(s, r#"{"kind":"paraphrase","mode":"rewrite"}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), req);
    }

    #[test]
    fn response_ok_roundtrip() {
        let resp = Response::Ok { generated_chars: 42, ms: 900 };
        let s = serde_json::to_string(&resp).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&s).unwrap(), resp);
    }

    #[test]
    fn response_error_roundtrip() {
        let resp = Response::Error {
            error_kind: ErrorKind::Inference,
            message: "boom".into(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        assert!(s.contains(r#""error_kind":"inference""#), "got: {s}");
        assert_eq!(serde_json::from_str::<Response>(&s).unwrap(), resp);
    }
}
