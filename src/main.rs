mod controller;
mod error;
mod loader;
mod model;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let path = match ui::resolve_tasks_path() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    match ui::run(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
