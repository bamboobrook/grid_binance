fn main() {
    println!(
        "{}",
        serde_json::to_string_pretty(&r24_engine::arithmetic_canaries()).unwrap()
    );
}
