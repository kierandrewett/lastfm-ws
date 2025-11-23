use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NowPlaying {
    pub id: String,
    pub artist: String,
    pub track: String,
    pub album: Option<String>,
    pub album_art: Option<String>,
    pub now_playing: bool,
    pub track_info: Option<Track>,
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub url: String,
    pub duration: String,
    pub artist: Artist,
    pub album: Option<Album>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub artist: String,
    pub title: String,
    pub url: String,
    pub image: Vec<Image>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    #[serde(rename(deserialize = "#text"))]
    pub url: String,
    pub size: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}