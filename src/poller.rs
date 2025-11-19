use crate::{bus::NowPlayingBus, lastfm::LastFmClient, model::NowPlaying, redis::RedisStore};
use tokio::time::{sleep, Duration};

pub async fn run_poller(client: LastFmClient, bus: NowPlayingBus, redis: RedisStore) {
    let mut last: Option<NowPlaying> = None;

    loop {
        if let Ok(Some(current)) = client.get_now_playing().await {
            if last.as_ref() != Some(&current) {
                if current.now_playing {
                    let _ = redis.set_now_playing(&current).await;
                    bus.publish(current.clone());
                } else {
                    let _ = redis.clear_now_playing().await;
                }
                last = Some(current);
            }
        }

        sleep(Duration::from_secs(5)).await;
    }
}