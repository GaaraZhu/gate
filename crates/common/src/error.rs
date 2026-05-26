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
}
