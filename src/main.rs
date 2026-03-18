mod controller;
mod error;
mod loader;
mod model;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let path = ui::resolve_tasks_path();

    match ui::run(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
