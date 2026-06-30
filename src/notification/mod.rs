pub trait Notify: Send + Sync {
    fn notify(&self, key: &FoundKey);
}

pub struct Notifier {
    telegram_token: Option<String>,
    telegram_chat_id: Option<String>,
    log_file: Option<std::path::PathBuf>,
}

#[derive(Clone, Debug)]
pub struct FoundKey {
    pub private_key: [u8; 32],
    pub public_key: [u8; 33],
    pub address: String,
    pub puzzle_id: u32,
    pub thread_id: u32,
    pub elapsed_seconds: u64,
}

impl Notify for Notifier {
    fn notify(&self, key: &FoundKey) {
        Notifier::notify(self, key);
    }
}

impl Notifier {
    pub fn new(telegram_token: Option<String>, telegram_chat_id: Option<String>, log_file: Option<std::path::PathBuf>) -> Self {
        Self { telegram_token, telegram_chat_id, log_file }
    }

    pub fn notify(&self, key: &FoundKey) {
        let msg = format!(
            "[FOUND] Puzzle #{} | Private Key: {} | Address: {} | Time: {}s | Thread: {}",
            key.puzzle_id,
            hex::encode(key.private_key),
            key.address,
            key.elapsed_seconds,
            key.thread_id,
        );

        println!("\n{}", "=".repeat(80));
        println!("  $$$ BITCOIN PRIVATE KEY FOUND $$$");
        println!("{}", "=".repeat(80));
        println!("  Puzzle         : #{}", key.puzzle_id);
        println!("  Private Key    : {}", hex::encode(key.private_key));
        println!("  Address        : {}", key.address);
        println!("  Time Elapsed   : {}s ({})", key.elapsed_seconds, format_duration(key.elapsed_seconds));
        println!("{}", "=".repeat(80));

        if let Some(path) = &self.log_file {
            if let Err(e) = std::fs::write(path, &msg) {
                eprintln!("[ERROR] Failed to write found key to file: {}", e);
            }
        }

        if let (Some(token), Some(chat_id)) = (&self.telegram_token, &self.telegram_chat_id) {
            self.send_telegram(token, chat_id, &msg);
        }
    }

    fn send_telegram(&self, token: &str, chat_id: &str, message: &str) {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap();

        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            token
        );

        let params = [
            ("chat_id", chat_id),
            ("text", message),
            ("parse_mode", "HTML"),
        ];

        match client.post(&url).form(&params).send() {
            Ok(_) => println!("[Telegram] Notification sent successfully"),
            Err(e) => eprintln!("[Telegram] Failed to send: {}", e),
        }
    }
}

fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let secs = secs % 60;
    if hours > 0 {
        format!("{}h {}m {}s", hours, mins, secs)
    } else if mins > 0 {
        format!("{}m {}s", mins, secs)
    } else {
        format!("{}s", secs)
    }
}
