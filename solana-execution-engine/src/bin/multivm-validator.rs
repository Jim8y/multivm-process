// Temporarily disabled due to compilation issues in multivm-agave
fn main() {
    eprintln!("agave-validator is temporarily disabled due to compilation issues in the multivm-agave repository");
    eprintln!("The fix-extract-if-rust-188 branch has a bug where test-only constants are used in non-test code");
    std::process::exit(1);
}
