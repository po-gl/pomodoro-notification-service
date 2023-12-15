use std::{sync::Arc, time::{SystemTime, UNIX_EPOCH}};
use log::debug;
use tokio::{sync::{mpsc, RwLock}, time::Duration};

use crate::models::TimerInterval;
use crate::types::{CancelMap, PushTokenMap};
use crate::apns::send_la_update_to_apns;
use crate::authtoken::AuthToken;
use crate::util;

pub async fn start_timing_loop(time_intervals: Vec<TimerInterval>,
    segment_count: u32,
    device_token: String,
    auth_token: Arc<RwLock<AuthToken>>,
    cancel_channels: Arc<RwLock<CancelMap>>,
    push_token_map: Arc<RwLock<PushTokenMap>>) {

    let mut current_time: Duration;
    let mut target_time: Duration;
    let mut wait_time: Duration;
    let short_device_token = util::get_short_token(&device_token);

    let (tx, mut rx) = mpsc::channel(1);
    { 
        let mut cancel_map = cancel_channels.write().await;
        if let Some(existing_cancel) = cancel_map.get(&device_token) {
            existing_cancel.send(true).await.ok();
        }
        cancel_map.insert(device_token.clone(), tx);
    }

    for time_interval in time_intervals {
        target_time = Duration::from_secs_f64(time_interval.starts_at);
        current_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        if target_time < current_time {
            // Set wait time to a second otherwise the APNs push might not deliver
            wait_time = Duration::from_secs(1)
        } else {
            wait_time = target_time - current_time
        }

        debug!("timing_loop:: short_device_token: ...{} waiting for {:?}", short_device_token, wait_time);

        let sleep_handle = tokio::time::sleep(wait_time);
        let cancel_handle = rx.recv();

        tokio::select! {
            _ = sleep_handle => {}
            _ = cancel_handle => {
            debug!("timing_loop:: short_device_token: ...{} request canceled", short_device_token);
            break;
            }
        }

        let auth = &auth_token.read().await.token;

        let push_token = push_token_map.read().await.get(&device_token).map(|v| v.clone());
        if let Some(push_token) = push_token {
            debug!("timing_loop:: short_device_token: ...{} AuthToken used: {}", short_device_token, auth);
            send_la_update_to_apns(&push_token, auth, &time_interval, segment_count).await;

        } else {
            // try again after a couple seconds if we don't yet have a push token
            tokio::time::sleep(Duration::from_secs(4)).await;

            let push_token = push_token_map.read().await.get(&device_token).map(|v| v.clone());
            if let Some(push_token) = push_token {
                send_la_update_to_apns(&push_token, auth, &time_interval, segment_count).await;
            }
        }
    }
}
