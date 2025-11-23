use crate::model::{NowPlaying, Track};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use sha1::Sha1;
use sha2::Digest;

pub struct LastFmClient {
    http: Client,
    api_key: String,
    user: String,
}

impl LastFmClient {
    pub fn new(api_key: String, user: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
            user,
        }
    }

    pub async fn get_now_playing(&self) -> Result<Option<NowPlaying>> {
        let url = "https://ws.audioscrobbler.com/2.0/";
        let res = self
            .http
            .get(url)
            .query(&[
                ("method", "user.getrecenttracks"),
                ("user", self.user.as_str()),
                ("api_key", self.api_key.as_str()),
                ("format", "json"),
                ("limit", "1"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let track = res["recenttracks"]["track"]
            .get(0)
            .cloned()
            .unwrap_or(Value::Null);

        if track.is_null() {
            return Ok(None);
        }

        let now_playing = track["@attr"]["nowplaying"]
            .as_str()
            .map(|v| v == "true")
            .unwrap_or(false);

        if !now_playing {
            return Ok(None);
        }
        

        let artist = track["artist"]["#text"].as_str().unwrap_or("").to_string();
        let name = track["name"].as_str().unwrap_or("").to_string();

        let id = format!(
            "{}-{}",
            artist.to_lowercase().replace(' ', "_"),
            name.to_lowercase().replace(' ', "_")
        );
        let id = Sha1::digest(id.as_bytes());
        let id = format!("{:x}", id);

        let album = track["album"]["#text"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let mut album_art = track["image"]
            .as_array()
            .and_then(|imgs| imgs.last())
            .and_then(|img| img["#text"].as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let timestamp = track["date"]["uts"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok());

        if album_art
            .as_ref()
            .map(|s| s.contains("2a96cbd8b46e442fc41c2b86b821562f"))
            .unwrap_or(false)
        {
            album_art = None;
        }

        let track_info = if now_playing {
            match self.get_track_info(&artist, &name).await {
                Ok(info) => Some(info),
                Err(e) => None,
            }
        } else {
            None
        };

        Ok(Some(NowPlaying {
            id,
            artist,
            track: name,
            album,
            album_art,
            now_playing,
            track_info,
            timestamp,
        }))
    }

    pub async fn get_track_info(&self, artist: &str, track: &str) -> Result<Track> {
        let url = "https://ws.audioscrobbler.com/2.0/";
        let res = self
            .http
            .get(url)
            .query(&[
                ("method", "track.getInfo"),
                ("artist", artist),
                ("track", track),
                ("api_key", self.api_key.as_str()),
                ("format", "json"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        let track_json = &res["track"];
        let track: Track = serde_json::from_value(track_json.clone())?;

        Ok(track)
    }
}
