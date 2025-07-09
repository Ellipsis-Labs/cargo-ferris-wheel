use miette::Result;

/// Main entry point for the ferris-wheel CLI tool
fn main() -> Result<()> {
    // Install miette's panic and error handler for beautiful error reporting
    miette::set_panic_hook();

    // Run the library's main function
    cargo_ferris_wheel::run()
}
