use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NowPlaying {
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub album_art: Option<String>,
    pub now_playing: bool,
}