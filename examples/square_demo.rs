fn main() {
    if let Err(err) = nu::run_square_demo() {
        eprintln!("square demo failed: {err}");
        std::process::exit(1);
    }
}
