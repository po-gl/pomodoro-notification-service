use std::collections::HashMap;
use tokio::sync::mpsc::Sender;

/// <device_token, Sender>
pub type CancelMap = HashMap<String, Sender<bool>>;

/// <device_token, push_token>
pub type PushTokenMap = HashMap<String, String>;
