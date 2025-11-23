use crate::{
    AppState,
    lastfm::LastFmClient,
    model::{NowPlaying, PlaybackState},
};
use tokio::time::{sleep, Duration};

pub async fn run_poller(client: LastFmClient, state: AppState) {
    let mut last_live: Option<NowPlaying> = None;

    loop {
        match client.get_now_playing().await {
            Ok(Some(current)) => {
                // --- there IS a live nowplaying track ---
                let mut playback = state.playback_state.write().await;
                let mut np_lock = state.now_playing.write().await;
                let mut started_lock = state.track_started_at.write().await;
                let mut last_pos_lock = state.last_position_ms.write().await;

                let is_same_track = last_live
                    .as_ref()
                    .map(|np| np.id == current.id)
                    .unwrap_or(false);

                match *playback {
                    PlaybackState::Playing => {
                        if !is_same_track {
                            // NEW SONG while already playing
                            println!("NEW SONG: {} - {}", current.artist, current.track);
                            *started_lock = Some(std::time::Instant::now());
                            *last_pos_lock = 0;
                            *np_lock = Some(current.clone());
                            state.bus.publish(current.clone());
                        } else {
                            // same song still playing, just refresh metadata
                            *np_lock = Some(current.clone());
                        }
                    }
                    PlaybackState::Paused => {
                        if is_same_track {
                            // RESUMED same song
                            println!("RESUMED: {} - {}", current.artist, current.track);
                            let offset = *last_pos_lock;
                            *started_lock =
                                Some(std::time::Instant::now() - Duration::from_millis(offset));
                            *np_lock = Some(current.clone());
                            *playback = PlaybackState::Playing;
                            state.bus.publish(current.clone());
                        } else {
                            // Paused some old track, now a new one started
                            println!("NEW SONG (from paused): {} - {}", current.artist, current.track);
                            *started_lock = Some(std::time::Instant::now());
                            *last_pos_lock = 0;
                            *np_lock = Some(current.clone());
                            *playback = PlaybackState::Playing;
                            state.bus.publish(current.clone());
                        }
                    }
                    PlaybackState::Stopped => {
                        // fresh start
                        println!("INITIAL / NEW SONG: {} - {}", current.artist, current.track);
                        *started_lock = Some(std::time::Instant::now());
                        *last_pos_lock = 0;
                        *np_lock = Some(current.clone());
                        *playback = PlaybackState::Playing;
                        state.bus.publish(current.clone());
                    }
                }

                last_live = Some(current);
            }

            Ok(None) => {
                // --- no live nowplaying track ---
                let mut playback = state.playback_state.write().await;
                let mut started_lock = state.track_started_at.write().await;
                let mut last_pos_lock = state.last_position_ms.write().await;
                let mut np_lock = state.now_playing.write().await;

                match *playback {
                    PlaybackState::Playing => {
                        // something WAS playing, now nowplaying vanished
                        if let (Some(ref last), Some(started_at)) =
                            (last_live.as_ref(), *started_lock)
                        {
                            let elapsed_ms = started_at.elapsed().as_millis() as u64;

                            let duration_ms = last
                                .track_info
                                .as_ref()
                                .and_then(|t| t.duration.parse::<u64>().ok())
                                .unwrap_or(0);

                            // small grace window
                            let near_end = duration_ms > 0
                                && elapsed_ms + 5_000 >= duration_ms; // 5s margin

                            if near_end {
                                // treat as FINISHED
                                println!(
                                    "FINISHED: {} - {} (elapsed={}ms, duration={}ms)",
                                    last.artist, last.track, elapsed_ms, duration_ms
                                );
                                *playback = PlaybackState::Stopped;
                                *started_lock = None;
                                *last_pos_lock = 0;
                                *np_lock = None;
                                // you *could* publish a "stopped" event here if you want
                            } else {
                                // treat as PAUSED / stopped early
                                println!(
                                    "PAUSED (no nowplaying, early stop): {} - {} at {}ms",
                                    last.artist, last.track, elapsed_ms
                                );
                                *last_pos_lock = elapsed_ms;

                                let paused = *last;
                                let mut paused_copy = paused.clone();
                                paused_copy.now_playing = false;

                                *np_lock = Some(paused.clone());
                                *playback = PlaybackState::Paused;
                                state.bus.publish(paused.clone());
                            }
                        } else {
                            // we were "playing" but have no state? just reset
                            *playback = PlaybackState::Stopped;
                            *started_lock = None;
                            *last_pos_lock = 0;
                            *np_lock = None;
                        }
                    }
                    PlaybackState::Paused | PlaybackState::Stopped => {
                        // stay paused/stopped, nothing to do
                    }
                }
            }

            Err(err) => {
                eprintln!("Last.fm poll error: {err}");
                // don't blow up on error, just chill
            }
        }

        sleep(Duration::from_secs(2)).await;
    }
}
