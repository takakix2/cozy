fn main() -> std::io::Result<()> {
    let filename = match std::env::args().nth(1) {
        Some(arg) if arg == "--version" || arg == "-V" => {
            println!("cozy {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        other => other,
    };
    cozy::run_cli(filename)
}
