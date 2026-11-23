use std::process::ExitCode;

fn main() -> ExitCode {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(err) => {
            eprintln!("error: failed to start async runtime: {err}");
            return ExitCode::FAILURE;
        }
    };

    match runtime.block_on(vmate_cli::entry()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // Print the full error chain so context and hints are visible.
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
