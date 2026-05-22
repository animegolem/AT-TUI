use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_image::{
    Resize, StatefulImage,
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
use reqwest::Client;
use sha2::{Digest, Sha256};

use crate::model::{FeedItem, ImageRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedImageProtocol {
    Auto,
    Kitty,
    Sixel,
    Iterm2,
    Halfblocks,
}

pub struct MediaCache {
    enabled: bool,
    cache_dir: PathBuf,
    http: Client,
    picker: Option<Picker>,
    images: HashMap<String, CachedImage>,
}

struct CachedImage {
    protocol: Option<StatefulProtocol>,
    error: Option<String>,
}

impl MediaCache {
    pub fn new(enabled: bool, requested: RequestedImageProtocol) -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "haiti-plan", "at-tui")
            .context("could not resolve cache directory")?;
        let cache_dir = dirs.cache_dir().join("images");
        fs::create_dir_all(&cache_dir)
            .with_context(|| format!("could not create {}", cache_dir.display()))?;

        let picker = if enabled {
            let mut picker = match requested {
                RequestedImageProtocol::Halfblocks => Picker::halfblocks(),
                _ => Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()),
            };
            match requested {
                RequestedImageProtocol::Auto => {}
                RequestedImageProtocol::Kitty => picker.set_protocol_type(ProtocolType::Kitty),
                RequestedImageProtocol::Sixel => picker.set_protocol_type(ProtocolType::Sixel),
                RequestedImageProtocol::Iterm2 => picker.set_protocol_type(ProtocolType::Iterm2),
                RequestedImageProtocol::Halfblocks => {
                    picker.set_protocol_type(ProtocolType::Halfblocks)
                }
            }
            Some(picker)
        } else {
            None
        };

        Ok(Self {
            enabled,
            cache_dir,
            http: Client::new(),
            picker,
            images: HashMap::new(),
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            cache_dir: PathBuf::new(),
            http: Client::new(),
            picker: None,
            images: HashMap::new(),
        }
    }

    pub fn protocol_name(&self) -> &'static str {
        match self.picker.as_ref().map(Picker::protocol_type) {
            Some(ProtocolType::Kitty) => "kitty",
            Some(ProtocolType::Sixel) => "sixel",
            Some(ProtocolType::Iterm2) => "iterm2",
            Some(ProtocolType::Halfblocks) => "halfblocks",
            None => "off",
        }
    }

    pub async fn ensure_item(&mut self, item: &FeedItem) {
        if !self.enabled {
            return;
        }

        for image in collect_images(item).into_iter().take(3) {
            let _ = self.ensure_url(&image.thumb_url).await;
        }

        if let Some(thumb_url) = item
            .external
            .as_ref()
            .and_then(|external| external.thumb_url.as_ref())
        {
            let _ = self.ensure_url(thumb_url).await;
        }
    }

    pub fn render_first_image(&mut self, frame: &mut Frame<'_>, area: Rect, item: &FeedItem) {
        let Some(url) = first_image_url(item) else {
            frame.render_widget(
                Paragraph::new("No image on selected post")
                    .block(Block::default().title("Media").borders(Borders::ALL)),
                area,
            );
            return;
        };

        let Some(cached) = self.images.get_mut(url) else {
            frame.render_widget(
                Paragraph::new("Image queued")
                    .block(Block::default().title("Media").borders(Borders::ALL)),
                area,
            );
            return;
        };

        if let Some(error) = &cached.error {
            frame.render_widget(
                Paragraph::new(error.clone())
                    .block(Block::default().title("Media").borders(Borders::ALL)),
                area,
            );
            return;
        }

        let Some(protocol) = cached.protocol.as_mut() else {
            frame.render_widget(
                Paragraph::new("Image unavailable")
                    .block(Block::default().title("Media").borders(Borders::ALL)),
                area,
            );
            return;
        };

        let block = Block::default().title("Media").borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_stateful_widget(
            StatefulImage::default().resize(Resize::Fit(None)),
            inner,
            protocol,
        );
    }

    async fn ensure_url(&mut self, url: &str) -> Result<()> {
        if self.images.contains_key(url) {
            return Ok(());
        }

        let result = self.download_and_decode(url).await;
        match result {
            Ok(protocol) => {
                self.images.insert(
                    url.to_owned(),
                    CachedImage {
                        protocol: Some(protocol),
                        error: None,
                    },
                );
            }
            Err(error) => {
                self.images.insert(
                    url.to_owned(),
                    CachedImage {
                        protocol: None,
                        error: Some(format!("Image failed: {error:#}")),
                    },
                );
            }
        }
        Ok(())
    }

    async fn download_and_decode(&mut self, url: &str) -> Result<StatefulProtocol> {
        let bytes = self
            .http
            .get(url)
            .send()
            .await
            .with_context(|| format!("could not download {url}"))?
            .error_for_status()
            .with_context(|| format!("image request failed for {url}"))?
            .bytes()
            .await
            .with_context(|| format!("could not read image bytes for {url}"))?;

        let path = self.cache_dir.join(cache_key(url));
        fs::write(&path, &bytes).with_context(|| format!("could not write {}", path.display()))?;
        let image = image::load_from_memory(&bytes)
            .with_context(|| format!("could not decode image from {url}"))?;
        let picker = self
            .picker
            .as_ref()
            .context("image rendering is disabled")?;
        Ok(picker.new_resize_protocol(image))
    }
}

fn collect_images(item: &FeedItem) -> Vec<&ImageRef> {
    let mut images = Vec::new();
    images.extend(item.images.iter());
    if let Some(quote) = &item.quote {
        images.extend(quote.images.iter());
    }
    images
}

fn first_image_url(item: &FeedItem) -> Option<&str> {
    item.images
        .first()
        .map(|image| image.thumb_url.as_str())
        .or_else(|| {
            item.quote
                .as_ref()
                .and_then(|quote| quote.images.first())
                .map(|image| image.thumb_url.as_str())
        })
        .or_else(|| item.external.as_ref()?.thumb_url.as_deref())
}

fn cache_key(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    format!("{}.img", hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable() {
        assert_eq!(
            cache_key("https://example.com/a.png"),
            cache_key("https://example.com/a.png")
        );
        assert_ne!(
            cache_key("https://example.com/a.png"),
            cache_key("https://example.com/b.png")
        );
    }
}
