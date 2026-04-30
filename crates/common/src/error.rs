use serde::Serialize;

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub fn exit_with_error(msg: &str) -> ! {
    let response = ErrorResponse {
        error: msg.to_string(),
    };
    println!("{}", serde_json::to_string(&response).unwrap());
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_to_error_key() {
        let resp = ErrorResponse {
            error: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"error":"something went wrong"}"#);
    }

    #[test]
    fn empty_message_serializes() {
        let resp = ErrorResponse {
            error: String::new(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"error":""}"#);
    }

    #[test]
    fn special_chars_are_escaped() {
        let resp = ErrorResponse {
            error: r#"path "foo" not found"#.to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["error"], r#"path "foo" not found"#);
    }
}
