fn main() {
    if let Err(err) = cloudreve_sync_sdk::run_ui() {
        eprintln!("Failed to start app: {}", err);
    }
}
