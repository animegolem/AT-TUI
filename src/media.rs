use std::{collections::HashMap, fs, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use image::DynamicImage;
use ratatui::{
    Frame,
    layout::Rect,
    widgets::{Block, BorderType, Borders, Paragraph},
};
use ratatui_image::{
    Resize, StatefulImage,
    picker::{Picker, ProtocolType},
    protocol::StatefulProtocol,
};
use reqwest::Client;
use sha2::{Digest, Sha256};

use crate::model::{FeedItem, VideoRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestedImageProtocol {
    Auto,
    Kitty,
    Sixel,
    Iterm2,
    Halfblocks,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewImage {
    pub url: String,
    pub alt: Option<String>,
    pub source: PreviewImageSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewVideo {
    pub playlist_url: String,
    pub thumb_url: Option<String>,
    pub alt: Option<String>,
    pub source: PreviewImageSource,
    pub cid: Option<String>,
    pub aspect_ratio: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewMedia {
    Image(PreviewImage),
    Video(PreviewVideo),
}

impl PreviewMedia {
    pub fn source_label(&self) -> &'static str {
        match self {
            Self::Image(image) => image.source.label(),
            Self::Video(video) => video.source.label(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewImageSource {
    Post,
    Quote,
}

impl PreviewImageSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Quote => "quote",
        }
    }
}

pub struct MediaCache {
    enabled: bool,
    cache_dir: PathBuf,
    http: Client,
    picker: Option<Picker>,
    images: HashMap<String, ImageState>,
    videos: HashMap<String, VideoState>,
}

enum ImageState {
    Loading,
    Ready(Box<StatefulProtocol>),
    Failed(String),
}

enum VideoState {
    Loading,
    Ready {
        frames: Vec<StatefulProtocol>,
        frame: usize,
    },
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ImageLoadJob {
    url: String,
    cache_dir: PathBuf,
    http: Client,
}

#[derive(Debug, Clone)]
pub struct VideoLoadJob {
    playlist_url: String,
    cache_dir: PathBuf,
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
            videos: HashMap::new(),
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            cache_dir: PathBuf::new(),
            http: Client::new(),
            picker: None,
            images: HashMap::new(),
            videos: HashMap::new(),
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

    pub fn should_load(&self, image: &PreviewImage) -> bool {
        self.enabled && !self.images.contains_key(&image.url)
    }

    pub fn mark_loading(&mut self, image: &PreviewImage) {
        if self.enabled {
            self.images
                .entry(image.url.clone())
                .or_insert(ImageState::Loading);
        }
    }

    pub fn load_job(&self, image: &PreviewImage) -> Option<ImageLoadJob> {
        self.enabled.then(|| ImageLoadJob {
            url: image.url.clone(),
            cache_dir: self.cache_dir.clone(),
            http: self.http.clone(),
        })
    }

    pub fn should_load_video(&self, video: &PreviewVideo) -> bool {
        self.enabled && !self.videos.contains_key(&video.playlist_url)
    }

    pub fn mark_video_loading(&mut self, video: &PreviewVideo) {
        if self.enabled {
            self.videos
                .entry(video.playlist_url.clone())
                .or_insert(VideoState::Loading);
        }
    }

    pub fn video_job(&self, video: &PreviewVideo) -> Option<VideoLoadJob> {
        self.enabled.then(|| VideoLoadJob {
            playlist_url: video.playlist_url.clone(),
            cache_dir: self.cache_dir.join("videos"),
        })
    }

    pub fn finish_load(&mut self, url: String, result: std::result::Result<DynamicImage, String>) {
        if !self.enabled {
            return;
        }

        let state = match result {
            Ok(image) => match self.picker.as_ref() {
                Some(picker) => ImageState::Ready(Box::new(picker.new_resize_protocol(image))),
                None => ImageState::Failed("Image rendering disabled".into()),
            },
            Err(error) => ImageState::Failed(error),
        };
        self.images.insert(url, state);
    }

    pub fn finish_video_load(
        &mut self,
        playlist_url: String,
        result: std::result::Result<Vec<DynamicImage>, String>,
    ) {
        if !self.enabled {
            return;
        }

        let state = match result {
            Ok(frames) if frames.is_empty() => {
                VideoState::Failed("Video did not produce terminal frames".into())
            }
            Ok(frames) => match self.picker.as_ref() {
                Some(picker) => VideoState::Ready {
                    frames: frames
                        .into_iter()
                        .map(|frame| picker.new_resize_protocol(frame))
                        .collect(),
                    frame: 0,
                },
                None => VideoState::Failed("Video rendering disabled".into()),
            },
            Err(error) => VideoState::Failed(error),
        };
        self.videos.insert(playlist_url, state);
    }

    pub fn state_name(&self, url: &str) -> &'static str {
        match self.images.get(url) {
            Some(ImageState::Loading) => "loading",
            Some(ImageState::Ready(_)) => "ready",
            Some(ImageState::Failed(_)) => "failed",
            None => "missing",
        }
    }

    pub fn video_state_name(&self, playlist_url: &str) -> &'static str {
        match self.videos.get(playlist_url) {
            Some(VideoState::Loading) => "loading",
            Some(VideoState::Ready { .. }) => "ready",
            Some(VideoState::Failed(_)) => "failed",
            None => "missing",
        }
    }

    pub fn advance_video(&mut self, playlist_url: &str) {
        if let Some(VideoState::Ready { frames, frame }) = self.videos.get_mut(playlist_url)
            && !frames.is_empty()
        {
            *frame = (*frame + 1) % frames.len();
        }
    }

    pub async fn ensure_item(&mut self, item: &FeedItem) {
        if !self.enabled {
            return;
        }

        for image in preview_images(item).into_iter().take(3) {
            let _ = self.ensure_url(&image.url).await;
        }

        if let Some(thumb_url) = item
            .external
            .as_ref()
            .and_then(|external| external.thumb_url.as_ref())
        {
            let _ = self.ensure_url(thumb_url).await;
        }
    }

    pub async fn ensure_images(&mut self, images: &[PreviewImage]) {
        if !self.enabled {
            return;
        }

        for image in images {
            let _ = self.ensure_url(&image.url).await;
        }
    }

    pub fn render_preview_image(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        image: &PreviewImage,
        title: impl Into<String>,
    ) {
        let title = title.into();
        if !self.enabled {
            frame.render_widget(
                Paragraph::new("Image rendering disabled").block(media_block(title)),
                area,
            );
            return;
        }

        let Some(cached) = self.images.get_mut(&image.url) else {
            frame.render_widget(
                Paragraph::new("Image queued").block(media_block(title)),
                area,
            );
            return;
        };

        let protocol = match cached {
            ImageState::Loading => {
                frame.render_widget(
                    Paragraph::new("Image loading").block(media_block(title)),
                    area,
                );
                return;
            }
            ImageState::Failed(error) => {
                frame.render_widget(
                    Paragraph::new(error.clone()).block(media_block(title)),
                    area,
                );
                return;
            }
            ImageState::Ready(protocol) => protocol.as_mut(),
        };

        let block = media_block(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_stateful_widget(
            StatefulImage::default().resize(Resize::Fit(None)),
            inner,
            protocol,
        );
    }

    pub fn render_preview_video(
        &mut self,
        frame: &mut Frame<'_>,
        area: Rect,
        video: &PreviewVideo,
        title: impl Into<String>,
    ) {
        let title = title.into();
        if !self.enabled {
            frame.render_widget(
                Paragraph::new("Video rendering disabled").block(media_block(title)),
                area,
            );
            return;
        }

        match self.videos.get_mut(&video.playlist_url) {
            Some(VideoState::Ready {
                frames,
                frame: frame_index,
            }) if !frames.is_empty() => {
                let block = media_block(title);
                let inner = block.inner(area);
                frame.render_widget(block, area);
                let frame_index = (*frame_index).min(frames.len() - 1);
                frame.render_stateful_widget(
                    StatefulImage::default().resize(Resize::Fit(None)),
                    inner,
                    &mut frames[frame_index],
                );
            }
            Some(VideoState::Loading) => {
                frame.render_widget(
                    Paragraph::new("Video decoding with ffmpeg").block(media_block(title)),
                    area,
                );
            }
            Some(VideoState::Failed(error)) => {
                frame.render_widget(
                    Paragraph::new(error.clone()).block(media_block(title)),
                    area,
                );
            }
            _ => {
                let message = if video.thumb_url.is_some() {
                    "Press Enter or p to decode terminal frames"
                } else {
                    "No thumbnail available. Press Enter or p to decode terminal frames"
                };
                frame.render_widget(Paragraph::new(message).block(media_block(title)), area);
            }
        }
    }

    async fn ensure_url(&mut self, url: &str) -> Result<()> {
        if self.images.contains_key(url) {
            return Ok(());
        }

        let result = self.download_and_decode(url).await;
        match result {
            Ok(protocol) => {
                self.images
                    .insert(url.to_owned(), ImageState::Ready(Box::new(protocol)));
            }
            Err(error) => {
                self.images.insert(
                    url.to_owned(),
                    ImageState::Failed(format!("Image failed: {error:#}")),
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

impl ImageLoadJob {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn run(self) -> (String, std::result::Result<DynamicImage, String>) {
        let url = self.url.clone();
        let result = self
            .download_and_decode()
            .await
            .map_err(|error| error.to_string());
        (url, result)
    }

    async fn download_and_decode(self) -> Result<DynamicImage> {
        let bytes = self
            .http
            .get(&self.url)
            .send()
            .await
            .with_context(|| format!("could not download {}", self.url))?
            .error_for_status()
            .with_context(|| format!("image request failed for {}", self.url))?
            .bytes()
            .await
            .with_context(|| format!("could not read image bytes for {}", self.url))?;

        let path = self.cache_dir.join(cache_key(&self.url));
        fs::write(&path, &bytes).with_context(|| format!("could not write {}", path.display()))?;
        image::load_from_memory(&bytes)
            .with_context(|| format!("could not decode image from {}", self.url))
    }
}

impl VideoLoadJob {
    pub async fn run(self) -> (String, std::result::Result<Vec<DynamicImage>, String>) {
        let playlist_url = self.playlist_url.clone();
        let result = tokio::task::spawn_blocking(move || self.decode_frames())
            .await
            .map_err(|error| error.to_string())
            .and_then(|result| result.map_err(|error| error.to_string()));
        (playlist_url, result)
    }

    fn decode_frames(self) -> Result<Vec<DynamicImage>> {
        fs::create_dir_all(&self.cache_dir)
            .with_context(|| format!("could not create {}", self.cache_dir.display()))?;
        let frame_dir = self.cache_dir.join(cache_stem(&self.playlist_url));
        fs::create_dir_all(&frame_dir)
            .with_context(|| format!("could not create {}", frame_dir.display()))?;

        let output = frame_dir.join("frame_%04d.jpg");
        let status = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-i")
            .arg(&self.playlist_url)
            .arg("-vf")
            .arg("fps=8,scale=640:-2:force_original_aspect_ratio=decrease")
            .arg("-vframes")
            .arg("120")
            .arg(&output)
            .status()
            .with_context(|| "could not run ffmpeg; install ffmpeg or open the video externally")?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "ffmpeg could not decode this video; open it externally with u"
            ));
        }

        let mut paths = fs::read_dir(&frame_dir)
            .with_context(|| format!("could not read {}", frame_dir.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("jpg"))
            .collect::<Vec<_>>();
        paths.sort();

        paths
            .into_iter()
            .take(120)
            .map(|path| {
                image::open(&path).with_context(|| format!("could not decode {}", path.display()))
            })
            .collect()
    }
}

pub fn preview_images(item: &FeedItem) -> Vec<PreviewImage> {
    let mut images = Vec::new();
    images.extend(item.images.iter().map(|image| {
        PreviewImage {
            url: image
                .fullsize_url
                .clone()
                .unwrap_or_else(|| image.thumb_url.clone()),
            alt: image.alt.clone(),
            source: PreviewImageSource::Post,
        }
    }));
    if let Some(quote) = &item.quote {
        images.extend(quote.images.iter().map(|image| {
            PreviewImage {
                url: image
                    .fullsize_url
                    .clone()
                    .unwrap_or_else(|| image.thumb_url.clone()),
                alt: image.alt.clone(),
                source: PreviewImageSource::Quote,
            }
        }));
    }
    images
}

pub fn preview_media(item: &FeedItem) -> Vec<PreviewMedia> {
    let mut media = preview_images(item)
        .into_iter()
        .map(PreviewMedia::Image)
        .collect::<Vec<_>>();
    media.extend(
        item.videos
            .iter()
            .map(|video| PreviewMedia::Video(preview_video(video, PreviewImageSource::Post))),
    );
    if let Some(quote) = &item.quote {
        media.extend(
            quote
                .videos
                .iter()
                .map(|video| PreviewMedia::Video(preview_video(video, PreviewImageSource::Quote))),
        );
    }
    media
}

fn preview_video(video: &VideoRef, source: PreviewImageSource) -> PreviewVideo {
    PreviewVideo {
        playlist_url: video.playlist_url.clone(),
        thumb_url: video.thumb_url.clone(),
        alt: video.alt.clone(),
        source,
        cid: video.cid.clone(),
        aspect_ratio: video.aspect_ratio,
    }
}

fn cache_key(url: &str) -> String {
    format!("{}.img", cache_stem(url))
}

fn cache_stem(url: &str) -> String {
    let digest = Sha256::digest(url.as_bytes());
    hex::encode(digest)
}

fn media_block(title: String) -> Block<'static> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ImageRef, QuotePost, VideoRef};

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

    #[test]
    fn collects_direct_images_before_quote_images() {
        let mut item = item();
        item.images = vec![ImageRef {
            thumb_url: "https://example.com/post-thumb.jpg".into(),
            fullsize_url: Some("https://example.com/post-full.jpg".into()),
            alt: Some("post alt".into()),
        }];
        item.quote = Some(QuotePost {
            uri: "quote".into(),
            cid: None,
            author_name: "Bob".into(),
            author_handle: "bob.test".into(),
            text: "quoted".into(),
            indexed_at: None,
            images: vec![ImageRef {
                thumb_url: "https://example.com/quote-thumb.jpg".into(),
                fullsize_url: None,
                alt: Some("quote alt".into()),
            }],
            videos: Vec::new(),
            external: None,
            links: Vec::new(),
            nested_quote: None,
        });

        let images = preview_images(&item);

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].url, "https://example.com/post-full.jpg");
        assert_eq!(images[0].source, PreviewImageSource::Post);
        assert_eq!(images[1].url, "https://example.com/quote-thumb.jpg");
        assert_eq!(images[1].source, PreviewImageSource::Quote);
    }

    #[test]
    fn image_state_moves_from_missing_to_loading_to_failed() {
        let mut cache = MediaCache::disabled();
        let image = PreviewImage {
            url: "https://example.com/image.jpg".into(),
            alt: None,
            source: PreviewImageSource::Post,
        };

        assert_eq!(cache.state_name(&image.url), "missing");
        cache.enabled = true;
        cache.mark_loading(&image);
        assert_eq!(cache.state_name(&image.url), "loading");
        cache.finish_load(image.url.clone(), Err("network failed".into()));
        assert_eq!(cache.state_name(&image.url), "failed");
    }

    #[test]
    fn preview_media_includes_images_then_videos() {
        let mut item = item();
        item.images = vec![ImageRef {
            thumb_url: "https://example.com/post-thumb.jpg".into(),
            fullsize_url: None,
            alt: None,
        }];
        item.videos = vec![VideoRef {
            playlist_url: "https://example.com/video.m3u8".into(),
            thumb_url: Some("https://example.com/video.jpg".into()),
            alt: Some("video".into()),
            cid: Some("cid".into()),
            aspect_ratio: Some((1, 1)),
        }];

        let media = preview_media(&item);

        assert!(matches!(media[0], PreviewMedia::Image(_)));
        assert!(matches!(media[1], PreviewMedia::Video(_)));
    }

    fn item() -> FeedItem {
        FeedItem {
            uri: "post".into(),
            cid: None,
            viewer_like: None,
            viewer_repost: None,
            author_did: None,
            author_name: "Alice".into(),
            author_handle: "alice.test".into(),
            author_following: None,
            avatar_url: None,
            text: "hello".into(),
            indexed_at: None,
            reply_count: 0,
            repost_count: 0,
            like_count: 0,
            quote_count: 0,
            images: Vec::new(),
            videos: Vec::new(),
            external: None,
            links: Vec::new(),
            quote: None,
            reason: None,
            reply: None,
            reply_root: None,
            embed_status: None,
            depth: 0,
        }
    }
}
