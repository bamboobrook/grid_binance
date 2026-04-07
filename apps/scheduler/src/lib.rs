pub mod jobs {
    pub mod membership_grace;
    pub mod reminders;
    pub mod symbol_sync;
}

#[cfg(test)]
pub mod test_support {
    use std::sync::{Mutex, OnceLock};

    pub fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
