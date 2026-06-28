mod buffer;
#[allow(dead_code)]
mod file;
#[allow(dead_code)]
mod input;
#[allow(dead_code)]
mod viewport;
#[allow(dead_code)]
mod renderer;
#[allow(dead_code)]
mod config;
mod cli;

fn main() -> std::process::ExitCode {
    cli::main()
}
