use crate::model::NowPlaying;
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct NowPlayingBus {
    pub tx: broadcast::Sender<NowPlaying>,
}

impl NowPlayingBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(32);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<NowPlaying> {
        self.tx.subscribe()
    }

    pub fn publish(&self, np: NowPlaying) {
        let _ = self.tx.send(np);
    }
}