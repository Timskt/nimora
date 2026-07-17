use nimora_user_code_host::WorkerMessage;
use std::io::{self, BufRead, Write};

fn main() {
    let mut line = String::new();
    let response = match io::stdin().lock().read_line(&mut line) {
        Ok(0) => WorkerMessage::Error {
            code: "protocol-error".to_owned(),
            message: "worker received no request".to_owned(),
        },
        Ok(_) => match serde_json::from_str::<WorkerMessage>(&line) {
            Ok(message) => nimora_user_code_worker::execute(message),
            Err(error) => WorkerMessage::Error {
                code: "protocol-error".to_owned(),
                message: error.to_string(),
            },
        },
        Err(error) => WorkerMessage::Error {
            code: "io-error".to_owned(),
            message: error.to_string(),
        },
    };
    if let Ok(encoded) = serde_json::to_string(&response) {
        let _ = writeln!(io::stdout().lock(), "{encoded}");
    }
}
