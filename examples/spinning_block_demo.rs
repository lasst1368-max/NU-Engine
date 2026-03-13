fn main() {
    if let Err(err) = nu::run_spinning_block_demo() {
        eprintln!("spinning block demo failed: {err}");
        std::process::exit(1);
    }
}
