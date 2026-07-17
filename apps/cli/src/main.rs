use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    match nimora_cli::run(&arguments) {
        Ok(output) => {
            println!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{}", error.json());
            ExitCode::from(error.code())
        }
    }
}
