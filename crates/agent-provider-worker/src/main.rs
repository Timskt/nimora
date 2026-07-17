use nimora_agent_provider_worker::execute_json;
use nimora_agent_runtime::{ProviderError, ProviderErrorKind};
use serde_json::json;
use std::io::{self, Read, Write};

fn main() {
    let mut input = Vec::new();
    let response = match io::stdin().take(1024 * 1024 + 1).read_to_end(&mut input) {
        Ok(_) => execute_json(&input).unwrap_or_else(|error| error_document(&error)),
        Err(_) => error_document(&ProviderError::new(
            ProviderErrorKind::InvalidRequest,
            "worker input failed",
        )),
    };
    let _ = io::stdout().write_all(&response);
}

fn error_document(error: &ProviderError) -> Vec<u8> {
    serde_json::to_vec(&json!({"type": "error", "error": error})).unwrap_or_default()
}
