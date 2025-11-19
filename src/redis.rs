use crate::model::NowPlaying;
use anyhow::Result;
use redis::{AsyncCommands, Client};

#[derive(Clone)]
pub struct RedisStore {
    client: Client,
}

impl RedisStore {
    pub fn new(url: &str) -> Self {
        Self {
            client: Client::open(url).unwrap(),
        }
    }

    pub async fn set_now_playing(&self, np: &NowPlaying) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let json = serde_json::to_string(np)?;
        let _: () = conn.set("lastfm:nowplaying", json).await?;
        Ok(())
    }

    pub async fn get_now_playing(&self) -> Result<Option<NowPlaying>> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let val: Option<String> = conn.get("lastfm:nowplaying").await?;
        Ok(val.and_then(|v| serde_json::from_str(&v).ok()))
    }

    pub async fn clear_now_playing(&self) -> Result<()> {
        let mut conn = self.client.get_multiplexed_async_connection().await?;
        let _: () = conn.del("lastfm:nowplaying").await?;
        Ok(())
    }
}