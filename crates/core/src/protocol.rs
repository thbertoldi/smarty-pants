//! Wire types shared between daemon and CLI.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Request {
    Paraphrase { mode: String },
    Status,
    Shutdown,
    Reload,
    SetPaused { paused: bool },
    UnloadModel,
    Configure { change: SettingChange },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "setting", rename_all = "snake_case")]
pub enum SettingChange {
    Provider {
        provider: crate::config::Provider,
    },
    LocalModel {
        name: String,
    },
    Gpu {
        enabled: bool,
    },
    KeepLoaded {
        enabled: bool,
    },
    RestoreClipboard {
        enabled: bool,
    },
    Delivery {
        delivery: crate::config::Delivery,
    },
    Paused {
        paused: bool,
    },
    ApiConnection {
        provider: crate::config::Provider,
        base_url: String,
        model: String,
        api_key_env: Option<String>,
        #[serde(default)]
        activate: bool,
    },
    ApiKeyFile {
        provider: crate::config::Provider,
        path: String,
    },
    ApiModel {
        provider: crate::config::Provider,
        model: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Response {
    Ok {
        generated_chars: usize,
        ms: u64,
    },
    Copied {
        generated_chars: usize,
        ms: u64,
        reason: String,
    },
    Cancelled,
    Empty,
    Busy,
    Paused,
    ModelLoading,
    Status {
        healthy: bool,
        model_loaded: bool,
        mode_count: usize,
        #[serde(default)]
        provider: String,
        #[serde(default)]
        model: String,
        #[serde(default)]
        paused: bool,
        #[serde(default)]
        busy: bool,
        #[serde(default)]
        last_error: Option<String>,
        #[serde(default)]
        last_result: Option<String>,
    },
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
    Configuration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_paraphrase_roundtrip() {
        let req = Request::Paraphrase {
            mode: "rewrite".into(),
        };
        let s = serde_json::to_string(&req).unwrap();
        assert_eq!(s, r#"{"kind":"paraphrase","mode":"rewrite"}"#);
        assert_eq!(serde_json::from_str::<Request>(&s).unwrap(), req);
    }

    #[test]
    fn response_ok_roundtrip() {
        let resp = Response::Ok {
            generated_chars: 42,
            ms: 900,
        };
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
