use std::{
    collections::{HashMap, VecDeque},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
    /// Smaller variant rendered while the fullsize `url` is still loading.
    pub thumb_url: Option<String>,
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

const IMAGE_CACHE_CAP: usize = 48;
const VIDEO_CACHE_CAP: usize = 4;

pub struct MediaCache {
    enabled: bool,
    cache_dir: PathBuf,
    http: Client,
    picker: Option<Picker>,
    images: HashMap<String, ImageState>,
    videos: HashMap<String, VideoState>,
    // Access-ordered keys backing the LRU bound; disk cache makes re-loads cheap.
    image_order: VecDeque<String>,
    video_order: VecDeque<String>,
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
            image_order: VecDeque::new(),
            video_order: VecDeque::new(),
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
            image_order: VecDeque::new(),
            video_order: VecDeque::new(),
        }
    }

    #[cfg(test)]
    fn test_enabled(cache_dir: PathBuf) -> Self {
        Self {
            enabled: true,
            cache_dir,
            http: Client::new(),
            picker: Some(Picker::halfblocks()),
            images: HashMap::new(),
            videos: HashMap::new(),
            image_order: VecDeque::new(),
            video_order: VecDeque::new(),
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
        self.should_load_url(&image.url)
    }

    pub fn mark_loading(&mut self, image: &PreviewImage) {
        self.mark_loading_url(&image.url);
    }

    pub fn load_job(&self, image: &PreviewImage) -> Option<ImageLoadJob> {
        self.load_job_url(&image.url)
    }

    pub fn should_load_url(&self, url: &str) -> bool {
        self.enabled && !self.images.contains_key(url)
    }

    pub fn mark_loading_url(&mut self, url: &str) {
        if self.enabled && !self.images.contains_key(url) {
            self.record_image(url.to_owned(), ImageState::Loading);
        }
    }

    pub fn load_job_url(&self, url: &str) -> Option<ImageLoadJob> {
        self.enabled.then(|| ImageLoadJob {
            url: url.to_owned(),
            cache_dir: self.cache_dir.clone(),
            http: self.http.clone(),
        })
    }

    fn record_image(&mut self, url: String, state: ImageState) {
        if self.images.insert(url.clone(), state).is_none() {
            self.image_order.push_back(url);
        }
        while self.image_order.len() > IMAGE_CACHE_CAP {
            if let Some(evicted) = self.image_order.pop_front() {
                self.images.remove(&evicted);
            }
        }
    }

    /// Move a key to the back of the eviction queue on render access.
    fn touch_image(&mut self, url: &str) {
        if let Some(position) = self.image_order.iter().position(|key| key == url) {
            let key = self
                .image_order
                .remove(position)
                .expect("position is in bounds");
            self.image_order.push_back(key);
        }
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
        self.record_image(url, state);
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
        if self.videos.insert(playlist_url.clone(), state).is_none() {
            self.video_order.push_back(playlist_url);
        }
        while self.video_order.len() > VIDEO_CACHE_CAP {
            if let Some(evicted) = self.video_order.pop_front() {
                self.videos.remove(&evicted);
            }
        }
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

    /// Which cached entry should draw for this image: the fullsize URL when
    /// ready, the thumbnail while fullsize is still in flight, or a message.
    fn image_render_key<'a>(
        &self,
        image: &'a PreviewImage,
    ) -> std::result::Result<&'a str, String> {
        if matches!(self.images.get(&image.url), Some(ImageState::Ready(_))) {
            return Ok(&image.url);
        }
        if let Some(thumb) = image.thumb_url.as_deref()
            && matches!(self.images.get(thumb), Some(ImageState::Ready(_)))
        {
            return Ok(thumb);
        }
        Err(match self.images.get(&image.url) {
            Some(ImageState::Loading) => "Image loading".into(),
            Some(ImageState::Failed(error)) => error.clone(),
            Some(ImageState::Ready(_)) => unreachable!("handled above"),
            None => "Image queued".into(),
        })
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

        let key = match self.image_render_key(image) {
            Ok(key) => key.to_owned(),
            Err(message) => {
                frame.render_widget(Paragraph::new(message).block(media_block(title)), area);
                return;
            }
        };

        self.touch_image(&key);
        let Some(ImageState::Ready(protocol)) = self.images.get_mut(&key) else {
            return;
        };

        let block = media_block(title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_stateful_widget(
            StatefulImage::default().resize(Resize::Fit(None)),
            inner,
            protocol.as_mut(),
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
}

impl ImageLoadJob {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn run(self) -> (String, std::result::Result<DynamicImage, String>) {
        let url = self.url.clone();
        let result = self.load().await.map_err(|error| error.to_string());
        (url, result)
    }

    async fn load(self) -> Result<DynamicImage> {
        let path = self.cache_dir.join(cache_key(&self.url));
        if let Some(image) = read_cached_image(&path).await {
            return Ok(image);
        }

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

        // Best effort: a failed cache write should not fail the render.
        let _ = fs::write(&path, &bytes);
        decode_image_bytes(bytes.to_vec(), self.url).await
    }
}

async fn read_cached_image(path: &Path) -> Option<DynamicImage> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || {
        let bytes = fs::read(&path).ok()?;
        match image::load_from_memory(&bytes) {
            Ok(image) => Some(image),
            Err(_) => {
                // Corrupt cache entry: drop it and fall back to the network.
                let _ = fs::remove_file(&path);
                None
            }
        }
    })
    .await
    .ok()
    .flatten()
}

async fn decode_image_bytes(bytes: Vec<u8>, url: String) -> Result<DynamicImage> {
    tokio::task::spawn_blocking(move || {
        image::load_from_memory(&bytes)
            .with_context(|| format!("could not decode image from {url}"))
    })
    .await?
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

fn preview_image_from_ref(
    image: &crate::model::ImageRef,
    source: PreviewImageSource,
) -> PreviewImage {
    let url = image
        .fullsize_url
        .clone()
        .unwrap_or_else(|| image.thumb_url.clone());
    let thumb_url = (url != image.thumb_url).then(|| image.thumb_url.clone());
    PreviewImage {
        url,
        thumb_url,
        alt: image.alt.clone(),
        source,
    }
}

pub fn preview_images(item: &FeedItem) -> Vec<PreviewImage> {
    let mut images = item
        .images
        .iter()
        .map(|image| preview_image_from_ref(image, PreviewImageSource::Post))
        .collect::<Vec<_>>();
    if let Some(quote) = &item.quote {
        images.extend(
            quote
                .images
                .iter()
                .map(|image| preview_image_from_ref(image, PreviewImageSource::Quote)),
        );
    }
    images
}

/// Thumbnail URLs worth warming while the cursor is near this post, so the
/// media overlay opens with something already decoded.
pub fn prefetch_thumb_urls(item: &FeedItem) -> Vec<String> {
    let mut urls: Vec<String> = item
        .images
        .iter()
        .map(|image| image.thumb_url.clone())
        .collect();
    urls.extend(
        item.videos
            .iter()
            .filter_map(|video| video.thumb_url.clone()),
    );
    urls.extend(
        item.external
            .as_ref()
            .and_then(|external| external.thumb_url.clone()),
    );
    if let Some(quote) = &item.quote {
        urls.extend(quote.images.iter().map(|image| image.thumb_url.clone()));
        urls.extend(
            quote
                .videos
                .iter()
                .filter_map(|video| video.thumb_url.clone()),
        );
        urls.extend(
            quote
                .external
                .as_ref()
                .and_then(|external| external.thumb_url.clone()),
        );
    }
    urls
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
        assert_eq!(
            images[0].thumb_url.as_deref(),
            Some("https://example.com/post-thumb.jpg")
        );
        assert_eq!(images[0].source, PreviewImageSource::Post);
        assert_eq!(images[1].url, "https://example.com/quote-thumb.jpg");
        assert_eq!(images[1].thumb_url, None);
        assert_eq!(images[1].source, PreviewImageSource::Quote);
    }

    fn tiny_image() -> DynamicImage {
        DynamicImage::ImageRgb8(image::RgbImage::new(2, 2))
    }

    #[tokio::test]
    async fn load_job_reads_disk_cache_before_network() {
        let dir = tempfile::tempdir().unwrap();
        // An unroutable URL proves a hit never touches the network.
        let url = "https://cache-hit.invalid/image.png";
        let path = dir.path().join(cache_key(url));
        tiny_image()
            .save_with_format(&path, image::ImageFormat::Png)
            .unwrap();

        let job = ImageLoadJob {
            url: url.into(),
            cache_dir: dir.path().to_owned(),
            http: Client::new(),
        };

        let (returned_url, result) = job.run().await;
        assert_eq!(returned_url, url);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn corrupt_cache_entry_is_discarded() {
        let dir = tempfile::tempdir().unwrap();
        let url = "https://cache-corrupt.invalid/image.png";
        let path = dir.path().join(cache_key(url));
        fs::write(&path, b"not an image").unwrap();

        assert!(read_cached_image(&path).await.is_none());
        assert!(!path.exists());
    }

    #[test]
    fn decoded_image_cache_is_lru_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = MediaCache::test_enabled(dir.path().to_owned());

        for index in 0..(IMAGE_CACHE_CAP + 2) {
            cache.finish_load(format!("https://example.com/{index}.png"), Ok(tiny_image()));
        }

        assert_eq!(cache.images.len(), IMAGE_CACHE_CAP);
        assert_eq!(cache.state_name("https://example.com/0.png"), "missing");
        assert_eq!(cache.state_name("https://example.com/1.png"), "missing");
        assert_eq!(cache.state_name("https://example.com/2.png"), "ready");

        // Touching the oldest survivor protects it from the next eviction.
        cache.touch_image("https://example.com/2.png");
        cache.finish_load("https://example.com/extra.png".into(), Ok(tiny_image()));
        assert_eq!(cache.state_name("https://example.com/2.png"), "ready");
        assert_eq!(cache.state_name("https://example.com/3.png"), "missing");
    }

    #[test]
    fn render_key_prefers_fullsize_then_thumb() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = MediaCache::test_enabled(dir.path().to_owned());
        let image = PreviewImage {
            url: "https://example.com/full.png".into(),
            thumb_url: Some("https://example.com/thumb.png".into()),
            alt: None,
            source: PreviewImageSource::Post,
        };

        assert_eq!(
            cache.image_render_key(&image),
            Err("Image queued".to_owned())
        );

        cache.mark_loading_url(&image.url);
        cache.finish_load("https://example.com/thumb.png".into(), Ok(tiny_image()));
        assert_eq!(
            cache.image_render_key(&image),
            Ok("https://example.com/thumb.png")
        );

        cache.finish_load(image.url.clone(), Ok(tiny_image()));
        assert_eq!(
            cache.image_render_key(&image),
            Ok("https://example.com/full.png")
        );
    }

    #[test]
    fn prefetch_urls_cover_post_quote_and_external_thumbs() {
        let mut item = item();
        item.images = vec![ImageRef {
            thumb_url: "https://example.com/post-thumb.jpg".into(),
            fullsize_url: Some("https://example.com/post-full.jpg".into()),
            alt: None,
        }];
        item.external = Some(crate::model::ExternalRef {
            uri: "https://example.com/article".into(),
            title: "article".into(),
            description: None,
            thumb_url: Some("https://example.com/card-thumb.jpg".into()),
        });

        let urls = prefetch_thumb_urls(&item);

        assert_eq!(
            urls,
            vec![
                "https://example.com/post-thumb.jpg".to_owned(),
                "https://example.com/card-thumb.jpg".to_owned(),
            ]
        );
    }

    #[test]
    fn image_state_moves_from_missing_to_loading_to_failed() {
        let mut cache = MediaCache::disabled();
        let image = PreviewImage {
            url: "https://example.com/image.jpg".into(),
            thumb_url: None,
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
            author_following_uri: None,
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
