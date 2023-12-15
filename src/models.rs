use serde::Deserialize;

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RequestData {
    pub time_intervals: Vec<TimerInterval>,
    pub segment_count: u32,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TimerInterval {
    pub status: String,
    pub task: String,
    pub starts_at: f64,
    pub current_segment: u32,
    pub alert: Alert,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    pub title: String,
    pub body: String,
    pub sound: String,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PushTokenData {
    pub push_token: String,
}
