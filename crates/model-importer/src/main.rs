use nimora_model_importer::{ModelProbeRequest, probe_staged_model};
use serde::Serialize;
use std::{env, io, path::Path};

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WorkerResponse<T> {
    Result { report: T },
    Error { code: &'static str, message: String },
}

fn main() {
    let response = run().unwrap_or_else(|message| WorkerResponse::Error {
        code: "invalid_request",
        message,
    });
    if serde_json::to_writer(io::stdout().lock(), &response).is_err() {
        std::process::exit(2);
    }
}

fn run() -> Result<WorkerResponse<nimora_model_importer::ModelProbeReport>, String> {
    let request = env::args_os()
        .nth(1)
        .ok_or_else(|| "missing JSON request argument".to_owned())?;
    let request: ModelProbeRequest =
        serde_json::from_str(&request.to_string_lossy()).map_err(|error| error.to_string())?;
    match probe_staged_model(Path::new("."), &request) {
        Ok(report) => Ok(WorkerResponse::Result { report }),
        Err(error) => Ok(WorkerResponse::Error {
            code: "model_rejected",
            message: error.to_string(),
        }),
    }
}
