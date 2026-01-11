#![allow(non_camel_case_types)]
use chrono::{Utc, SecondsFormat};

pub struct logger;

impl logger {
    fn get_timestamp() -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }

    pub fn info(message: &str) {
        println!("[INFO] {} - {}", Self::get_timestamp(), message);
    }
    pub fn warn(message: &str) {
        eprintln!("[WARN] {} - {}", Self::get_timestamp(), message);
    }
    pub fn error(message: &str) {
        eprintln!("[ERROR] {} - {}", Self::get_timestamp(), message);
    }
}