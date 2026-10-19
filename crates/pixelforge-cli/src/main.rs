fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(pixelforge_cli::run() as u8)
}
