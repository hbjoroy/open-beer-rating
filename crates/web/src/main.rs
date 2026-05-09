mod app;
mod pages;
mod components;

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!("Error: open-tappd-web is a WASM application and cannot run natively.");
        eprintln!();
        eprintln!("To run the frontend, use one of these methods:");
        eprintln!();
        eprintln!("  1. Using trunk (recommended for development):");
        eprintln!("     cd crates/web && trunk serve");
        eprintln!();
        eprintln!("  2. Build WASM manually:");
        eprintln!("     cargo build -p open-tappd-web --target wasm32-unknown-unknown");
        eprintln!();
        std::process::exit(1);
    }

    #[cfg(target_arch = "wasm32")]
    {
        console_error_panic_hook::set_once();
        leptos::mount::mount_to_body(app::App);
    }
}
