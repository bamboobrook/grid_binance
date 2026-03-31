pub fn issue_email_code(seed: u64) -> String {
    format!("{:06}", seed % 1_000_000)
}

pub fn verify_email_code(expected: &str, actual: &str) -> bool {
    expected == actual
}
