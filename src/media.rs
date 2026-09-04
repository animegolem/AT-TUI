use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime},
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

use crate::{
    media_scheduler::{MediaExecutionLimits, MediaJobId},
    model::{FeedItem, VideoRef},
};

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
const DISK_CACHE_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const DISK_CACHE_MAX_BYTES: u64 = 256 * 1024 * 1024;
const MEDIA_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CACHE_MAINTENANCE_DELAY: Duration = Duration::from_millis(50);
const STAGING_PREFIX: &str = ".at-tui-stage-";
static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);
static ACTIVE_STAGING: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

pub struct MediaCache {
    enabled: bool,
    cache_dir: PathBuf,
    http: Client,
    picker: Option<Picker>,
    images: HashMap<String, ImageState>,
    loading_images: HashSet<String>,
    disk_cache: Arc<DiskCache>,
    // Access-ordered keys backing the LRU bound; disk cache makes re-loads cheap.
    image_order: VecDeque<String>,
}

enum ImageState {
    Ready(Box<StatefulProtocol>),
    Failed(String),
}

#[derive(Debug, Clone)]
pub struct ImageLoadJob {
    url: String,
    cache_dir: PathBuf,
    http: Client,
    disk_cache: Arc<DiskCache>,
}

impl MediaCache {
    pub fn new(enabled: bool, requested: RequestedImageProtocol) -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "haiti-plan", "at-tui")
            .context("could not resolve cache directory")?;
        Self::with_cache_dir(enabled, requested, dirs.cache_dir().join("images"))
    }

    fn with_cache_dir(
        enabled: bool,
        requested: RequestedImageProtocol,
        cache_dir: PathBuf,
    ) -> Result<Self> {
        let disk_cache =
            DiskCache::new(cache_dir.clone(), DISK_CACHE_MAX_AGE, DISK_CACHE_MAX_BYTES);
        // Construction is synchronous and is also usable without a Tokio
        // runtime. Missing/unwritable cache directories never disable display.
        disk_cache.request_maintenance();
        let http = Client::builder()
            .timeout(MEDIA_REQUEST_TIMEOUT)
            .build()
            .context("could not build media HTTP client")?;

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
            http,
            picker,
            images: HashMap::new(),
            loading_images: HashSet::new(),
            disk_cache,
            image_order: VecDeque::new(),
        })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            cache_dir: PathBuf::new(),
            http: Client::new(),
            picker: None,
            images: HashMap::new(),
            loading_images: HashSet::new(),
            disk_cache: DiskCache::new(PathBuf::new(), DISK_CACHE_MAX_AGE, DISK_CACHE_MAX_BYTES),
            image_order: VecDeque::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn test_enabled(cache_dir: PathBuf) -> Self {
        let disk_cache =
            DiskCache::new(cache_dir.clone(), DISK_CACHE_MAX_AGE, DISK_CACHE_MAX_BYTES);
        Self {
            enabled: true,
            cache_dir,
            http: Client::new(),
            picker: Some(Picker::halfblocks()),
            images: HashMap::new(),
            loading_images: HashSet::new(),
            disk_cache,
            image_order: VecDeque::new(),
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

    pub fn prepare_image_load(&mut self, url: &str, retry_failed: bool) -> bool {
        // The scheduler owns deduplication: loading work must still receive
        // demand updates so visible requests can promote or revive it.
        if !self.enabled {
            return false;
        }
        match self.images.get(url) {
            Some(ImageState::Ready(_)) => false,
            Some(ImageState::Failed(_)) if retry_failed => {
                self.images.remove(url);
                self.image_order.retain(|key| key != url);
                true
            }
            Some(ImageState::Failed(_)) => false,
            None => true,
        }
    }

    pub fn mark_loading_url(&mut self, url: &str) {
        if self.enabled && !self.images.contains_key(url) {
            self.loading_images.insert(url.to_owned());
        }
    }

    pub fn load_job_url(&self, url: &str) -> Option<ImageLoadJob> {
        self.enabled.then(|| ImageLoadJob {
            url: url.to_owned(),
            cache_dir: self.cache_dir.clone(),
            http: self.http.clone(),
            disk_cache: self.disk_cache.clone(),
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

    pub fn finish_load(&mut self, url: String, result: std::result::Result<DynamicImage, String>) {
        if !self.enabled {
            return;
        }
        self.loading_images.remove(&url);

        let state = match result {
            Ok(image) => match self.picker.as_ref() {
                Some(picker) => ImageState::Ready(Box::new(picker.new_resize_protocol(image))),
                None => ImageState::Failed("Image rendering disabled".into()),
            },
            Err(error) => ImageState::Failed(error),
        };
        self.record_image(url, state);
    }

    pub fn state_name(&self, url: &str) -> &'static str {
        if self.loading_images.contains(url) {
            return "loading";
        }
        match self.images.get(url) {
            Some(ImageState::Ready(_)) => "ready",
            Some(ImageState::Failed(_)) => "failed",
            None => "missing",
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
            _ if self.loading_images.contains(&image.url) => "Image loading".into(),
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
        let message = if video.thumb_url.is_some() {
            "Press Enter or p for mpv playback\nq returns to AT-TUI · u opens externally"
        } else {
            "No thumbnail available\nPress Enter or p for mpv playback · u opens externally"
        };
        frame.render_widget(Paragraph::new(message).block(media_block(title)), area);
    }

    pub fn cancel_loading(&mut self, id: &MediaJobId) {
        self.loading_images.remove(&id.source);
    }
}

impl ImageLoadJob {
    pub async fn run_limited(
        self,
        limits: MediaExecutionLimits,
    ) -> (String, std::result::Result<DynamicImage, String>) {
        let url = self.url.clone();
        let result = self
            .load_limited(limits)
            .await
            .map_err(|error| error.to_string());
        (url, result)
    }

    async fn load_limited(self, limits: MediaExecutionLimits) -> Result<DynamicImage> {
        let path = self.cache_dir.join(cache_key(&self.url));
        {
            let _decode = limits.decode_permit().await;
            if let Some(image) = read_cached_image(&path).await {
                return Ok(image);
            }
        }
        let bytes = {
            let _download = limits.download_permit().await;
            self.http
                .get(&self.url)
                .send()
                .await
                .with_context(|| format!("could not download {}", self.url))?
                .error_for_status()
                .with_context(|| format!("image request failed for {}", self.url))?
                .bytes()
                .await
                .with_context(|| format!("could not read image bytes for {}", self.url))?
        };
        self.disk_cache.publish(path, bytes.clone()).await;
        let _decode = limits.decode_permit().await;
        decode_image_bytes(bytes, self.url).await
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

async fn decode_image_bytes(
    bytes: impl AsRef<[u8]> + Send + 'static,
    url: String,
) -> Result<DynamicImage> {
    tokio::task::spawn_blocking(move || {
        image::load_from_memory(bytes.as_ref())
            .with_context(|| format!("could not decode image from {url}"))
    })
    .await?
}

#[derive(Debug, Default)]
struct MaintenanceState {
    requested: u64,
    running: bool,
}

#[derive(Debug)]
struct DiskCache {
    directory: PathBuf,
    max_age: Duration,
    max_bytes: u64,
    maintenance: Mutex<MaintenanceState>,
    #[cfg(test)]
    probe: MaintenanceProbe,
}

impl DiskCache {
    fn new(directory: PathBuf, max_age: Duration, max_bytes: u64) -> Arc<Self> {
        Arc::new(Self {
            directory,
            max_age,
            max_bytes,
            maintenance: Mutex::new(MaintenanceState::default()),
            #[cfg(test)]
            probe: MaintenanceProbe::default(),
        })
    }

    async fn publish(self: &Arc<Self>, path: PathBuf, bytes: impl AsRef<[u8]> + Send + 'static) {
        let cache = self.clone();
        // Await only the bounded job's publication, never directory maintenance.
        // Byte ownership makes concurrent eviction harmless to image decoding.
        let _ = tokio::task::spawn_blocking(move || {
            let _ = publish_cache_bytes(&cache.directory, &path, bytes.as_ref());
            // Even a failed write may leave cleanup work, so request a pass.
            cache.request_maintenance();
        })
        .await;
    }

    fn request_maintenance(self: &Arc<Self>) {
        let mut state = self.maintenance.lock().unwrap_or_else(|e| e.into_inner());
        state.requested = state.requested.saturating_add(1);
        if state.running {
            return;
        }
        state.running = true;
        drop(state);

        let cache = self.clone();
        if std::thread::Builder::new()
            .name("at-tui-cache-maintenance".into())
            .spawn(move || cache.run_maintenance())
            .is_err()
        {
            // Best effort: the next publication/startup request can retry.
            self.maintenance
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .running = false;
            #[cfg(test)]
            self.probe.idle.notify_all();
        }
    }

    fn run_maintenance(self: Arc<Self>) {
        loop {
            // Coalesce publications from a burst, including requests received
            // during the previous scan. The worker exits when quiescent.
            std::thread::sleep(CACHE_MAINTENANCE_DELAY);
            let generation = self
                .maintenance
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .requested;
            #[cfg(test)]
            self.probe.before_scan();
            cleanup_cache_dir(
                &self.directory,
                self.max_age,
                self.max_bytes,
                SystemTime::now(),
            );
            let mut state = self.maintenance.lock().unwrap_or_else(|e| e.into_inner());
            if state.requested == generation {
                // The comparison and idle transition share the request lock:
                // a publication can neither miss this pass nor lose the next.
                state.running = false;
                #[cfg(test)]
                self.probe.idle.notify_all();
                return;
            }
            // Failed scans do not spin: absent another request we also retire.
        }
    }
}

fn active_staging() -> &'static Mutex<HashSet<PathBuf>> {
    ACTIVE_STAGING.get_or_init(|| Mutex::new(HashSet::new()))
}

struct StagingFile {
    path: PathBuf,
    file: fs::File,
}

impl StagingFile {
    fn create(directory: &Path) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        // Canonical paths share protection across separately constructed caches
        // that refer to the same directory through different aliases.
        let directory = fs::canonicalize(directory)?;
        loop {
            let sequence = NEXT_STAGING_ID.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(
                "{STAGING_PREFIX}{}-{sequence}.tmp",
                std::process::id()
            ));
            let mut active = active_staging().lock().unwrap_or_else(|e| e.into_inner());
            active.insert(path.clone());
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(file) => return Ok(Self { path, file }),
                Err(error) => {
                    active.remove(&path);
                    if error.kind() != io::ErrorKind::AlreadyExists {
                        return Err(error);
                    }
                }
            }
        }
    }

    fn publish(self, destination: &Path) -> io::Result<()> {
        fs::rename(&self.path, destination)
    }
}

impl Drop for StagingFile {
    fn drop(&mut self) {
        // Keep protection until the partial file is gone. If removal fails,
        // releasing registration lets a later maintenance pass reclaim it.
        let mut active = active_staging().lock().unwrap_or_else(|e| e.into_inner());
        let _ = fs::remove_file(&self.path);
        active.remove(&self.path);
    }
}

fn publish_cache_bytes(directory: &Path, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut staging = StagingFile::create(directory)?;
    staging.file.write_all(bytes)?;
    staging.publish(path)
}

fn is_staging_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(STAGING_PREFIX))
}

fn remove_cache_file(path: &Path) -> bool {
    if is_staging_path(path) {
        // Registration precedes creation; deletion checks while holding the
        // same lock, so a concurrent scan cannot unlink a live partial write.
        let active = active_staging().lock().unwrap_or_else(|e| e.into_inner());
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());
        if active.contains(&canonical) {
            return false;
        }
        // Unknown staging (including crash leftovers) has no grace exemption.
        // Another process may lose a best-effort cache publication, while its
        // owned download bytes remain available for image display.
        return fs::remove_file(path).is_ok();
    }
    fs::remove_file(path).is_ok()
}

#[cfg(test)]
#[derive(Debug, Default)]
struct MaintenanceProbe {
    scans: Mutex<(usize, bool)>,
    changed: std::sync::Condvar,
    idle: std::sync::Condvar,
}

#[cfg(test)]
impl MaintenanceProbe {
    fn before_scan(&self) {
        let mut state = self.scans.lock().unwrap();
        state.0 += 1;
        self.changed.notify_all();
        while state.1 {
            state = self.changed.wait(state).unwrap();
        }
    }
}

#[derive(Debug)]
struct CacheFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
    removed: bool,
}

fn cleanup_cache_dir(cache_dir: &Path, max_age: Duration, max_bytes: u64, now: SystemTime) {
    let mut paths = Vec::new();
    collect_cache_files(cache_dir, &mut paths);
    let mut total_bytes = 0_u64;
    let mut candidates = Vec::new();
    for path in paths {
        let Ok(metadata) = fs::metadata(&path) else {
            continue;
        };
        let size = metadata.len();
        total_bytes = total_bytes.saturating_add(size);
        candidates.push(CacheFile {
            path,
            size,
            modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            removed: false,
        });
    }

    for candidate in &mut candidates {
        if (is_staging_path(&candidate.path)
            || now
                .duration_since(candidate.modified)
                .is_ok_and(|age| age > max_age))
            && remove_cache_file(&candidate.path)
        {
            candidate.removed = true;
            total_bytes = total_bytes.saturating_sub(candidate.size);
        }
    }

    candidates.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    for candidate in candidates {
        if total_bytes <= max_bytes {
            break;
        }
        if !candidate.removed && remove_cache_file(&candidate.path) {
            total_bytes = total_bytes.saturating_sub(candidate.size);
        }
    }
}

fn collect_cache_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_cache_files(&path, files);
        } else if path.is_file() {
            files.push(path);
        }
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
            disk_cache: DiskCache::new(
                dir.path().to_owned(),
                DISK_CACHE_MAX_AGE,
                DISK_CACHE_MAX_BYTES,
            ),
        };

        let limits = crate::media_scheduler::MediaScheduler::default().execution_limits();
        let (returned_url, result) = job.run_limited(limits).await;
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
    fn disk_cleanup_enforces_size_for_current_session_entries() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a");
        let second = dir.path().join("b");
        let current_path = dir.path().join("current");
        fs::write(&first, [0_u8; 5]).unwrap();
        fs::write(&second, [0_u8; 5]).unwrap();
        fs::write(&current_path, [0_u8; 5]).unwrap();

        cleanup_cache_dir(
            dir.path(),
            Duration::from_secs(365 * 24 * 60 * 60),
            8,
            SystemTime::now(),
        );

        let remaining_bytes = [first, second, current_path]
            .into_iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .sum::<u64>();
        assert_eq!(remaining_bytes, 5);
    }

    #[test]
    fn disk_cleanup_removes_expired_current_session_entries() {
        let dir = tempfile::tempdir().unwrap();
        let expired = dir.path().join("expired");
        let current_path = dir.path().join("current");
        fs::write(&expired, b"old").unwrap();
        fs::write(&current_path, b"current").unwrap();

        cleanup_cache_dir(
            dir.path(),
            Duration::from_secs(30 * 24 * 60 * 60),
            u64::MAX,
            SystemTime::now() + Duration::from_secs(31 * 24 * 60 * 60),
        );

        assert!(!expired.exists());
        assert!(!current_path.exists());
    }

    fn wait_for_maintenance(cache: &DiskCache) {
        let state = cache.maintenance.lock().unwrap();
        let (state, timeout) = cache
            .probe
            .idle
            .wait_timeout_while(state, Duration::from_secs(5), |state| state.running)
            .unwrap();
        assert!(
            !timeout.timed_out() && !state.running,
            "maintenance did not become idle"
        );
    }

    fn pause_maintenance(cache: &Arc<DiskCache>) {
        cache.probe.scans.lock().unwrap().1 = true;
        cache.request_maintenance();
        let state = cache.probe.scans.lock().unwrap();
        let (state, timeout) = cache
            .probe
            .changed
            .wait_timeout_while(state, Duration::from_secs(5), |state| state.0 == 0)
            .unwrap();
        assert!(!timeout.timed_out() && state.0 == 1, "scan did not start");
    }

    fn resume_maintenance(cache: &DiskCache) {
        cache.probe.scans.lock().unwrap().1 = false;
        cache.probe.changed.notify_all();
    }

    fn disk_bytes(directory: &Path) -> u64 {
        let mut paths = Vec::new();
        collect_cache_files(directory, &mut paths);
        paths
            .into_iter()
            .map(|path| fs::metadata(path).unwrap().len())
            .sum()
    }

    fn png_bytes() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        tiny_image()
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn local_image_server(bytes: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image.png", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0; 2048];
            assert!(stream.read(&mut request).unwrap() > 0);
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&bytes).unwrap();
        });
        (url, worker)
    }

    fn limited_job(url: String, cache: Arc<DiskCache>) -> ImageLoadJob {
        ImageLoadJob {
            url,
            cache_dir: cache.directory.clone(),
            http: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap(),
            disk_cache: cache,
        }
    }

    #[test]
    fn synchronous_cache_construction_schedules_startup_cleanup_without_runtime() {
        let dir = tempfile::tempdir().unwrap();
        let orphan = dir.path().join(format!("{STAGING_PREFIX}orphan.tmp"));
        fs::write(&orphan, b"abandoned").unwrap();
        let cache = MediaCache::with_cache_dir(
            true,
            RequestedImageProtocol::Halfblocks,
            dir.path().to_owned(),
        )
        .unwrap();
        wait_for_maintenance(&cache.disk_cache);
        assert!(!orphan.exists());
        assert_eq!(cache.protocol_name(), "halfblocks");
        assert_eq!(cache.disk_cache.probe.scans.lock().unwrap().0, 1);
    }

    #[test]
    fn oversized_entries_and_abandoned_staging_are_reclaimable() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("oversized.img"), [1; 12]).unwrap();
        fs::write(
            dir.path().join(format!("{STAGING_PREFIX}crashed.tmp")),
            [1; 12],
        )
        .unwrap();
        cleanup_cache_dir(dir.path(), DISK_CACHE_MAX_AGE, 8, SystemTime::now());
        assert_eq!(disk_bytes(dir.path()), 0);
    }

    #[test]
    fn atomic_publication_protects_live_staging_and_exposes_only_complete_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("image.img");
        fs::write(&destination, b"old").unwrap();
        let mut staging = StagingFile::create(dir.path()).unwrap();
        staging.file.write_all(b"new").unwrap();
        let staged_path = staging.path.clone();
        // A separately constructed cache uses the same process-wide registry.
        let other_cache = DiskCache::new(dir.path().join("."), DISK_CACHE_MAX_AGE, u64::MAX);
        other_cache.request_maintenance();
        wait_for_maintenance(&other_cache);
        assert_eq!(fs::read(&destination).unwrap(), b"old");
        assert_eq!(fs::read(&staged_path).unwrap(), b"new");
        staging.file.write_all(b" image").unwrap();
        staging.publish(&destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"new image");
        assert!(!staged_path.exists());
        assert!(!active_staging().lock().unwrap().contains(&staged_path));
        cleanup_cache_dir(dir.path(), DISK_CACHE_MAX_AGE, 0, SystemTime::now());
        assert_eq!(disk_bytes(dir.path()), 0);
    }

    #[test]
    fn live_staging_is_the_only_temporary_budget_exception() {
        let dir = tempfile::tempdir().unwrap();
        let mut staging = StagingFile::create(dir.path()).unwrap();
        staging.file.write_all(&[1; 12]).unwrap();
        let destination = dir.path().join("published.img");
        cleanup_cache_dir(
            dir.path(),
            Duration::ZERO,
            8,
            SystemTime::now() + Duration::from_secs(1),
        );
        assert_eq!(disk_bytes(dir.path()), 12);
        staging.publish(&destination).unwrap();
        cleanup_cache_dir(dir.path(), DISK_CACHE_MAX_AGE, 8, SystemTime::now());
        assert_eq!(disk_bytes(dir.path()), 0);
    }

    #[tokio::test]
    async fn burst_publications_coalesce_and_run_a_final_bounded_scan() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_owned(), DISK_CACHE_MAX_AGE, 8);
        pause_maintenance(&cache);
        let mut tasks = Vec::new();
        for index in 0..12 {
            let cache = cache.clone();
            tasks.push(tokio::spawn(async move {
                cache
                    .publish(cache.directory.join(format!("{index}.img")), vec![1; 6])
                    .await;
            }));
        }
        for task in tasks {
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap();
        }
        assert_eq!(disk_bytes(dir.path()), 72);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 1);
        resume_maintenance(&cache);
        wait_for_maintenance(&cache);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 2);
        assert_eq!(disk_bytes(dir.path()), 6);

        // A publication after the idle transition starts another worker/pass.
        cache.publish(dir.path().join("last.img"), vec![1; 6]).await;
        wait_for_maintenance(&cache);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 3);
        assert_eq!(disk_bytes(dir.path()), 6);
    }

    #[tokio::test]
    async fn production_image_result_does_not_wait_for_a_slow_scan() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_owned(), DISK_CACHE_MAX_AGE, 0);
        pause_maintenance(&cache);
        let (url, server) = local_image_server(png_bytes());
        let job = limited_job(url, cache.clone());
        let limits = crate::media_scheduler::MediaScheduler::default().execution_limits();
        let (_, result) = tokio::time::timeout(Duration::from_secs(5), job.run_limited(limits))
            .await
            .unwrap();
        server.join().unwrap();
        let image = result.unwrap();
        assert_eq!((image.width(), image.height()), (2, 2));
        assert!(disk_bytes(dir.path()) > 0);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 1);
        resume_maintenance(&cache);
        wait_for_maintenance(&cache);
        assert_eq!(disk_bytes(dir.path()), 0);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 2);
    }

    #[tokio::test]
    async fn production_loader_replaces_corrupt_cache_from_local_http() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_owned(), DISK_CACHE_MAX_AGE, u64::MAX);
        let (url, server) = local_image_server(png_bytes());
        let path = dir.path().join(cache_key(&url));
        fs::write(&path, b"not an image").unwrap();
        let limits = crate::media_scheduler::MediaScheduler::default().execution_limits();
        let (_, result) = limited_job(url, cache.clone()).run_limited(limits).await;
        server.join().unwrap();
        assert!(result.is_ok());
        wait_for_maintenance(&cache);
        assert!(image::load_from_memory(&fs::read(path).unwrap()).is_ok());
    }

    #[tokio::test]
    async fn cache_publication_and_maintenance_failures_do_not_fail_image_display() {
        let dir = tempfile::tempdir().unwrap();
        // A file where a cache directory belongs makes both writing and
        // directory scanning fail deterministically, even as an admin user.
        let directory = dir.path().join("not-a-directory");
        fs::write(&directory, b"occupied").unwrap();
        let cache = DiskCache::new(directory.clone(), DISK_CACHE_MAX_AGE, 0);
        let (url, server) = local_image_server(png_bytes());
        let limits = crate::media_scheduler::MediaScheduler::default().execution_limits();
        let (_, result) = limited_job(url, cache.clone()).run_limited(limits).await;
        server.join().unwrap();
        assert!(result.is_ok());
        wait_for_maintenance(&cache);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 1);
        // A later publication retries maintenance after the underlying failure
        // is repaired; failures never leave running=true or cause a hot loop.
        fs::remove_file(&directory).unwrap();
        cache
            .publish(directory.join("recovered.img"), vec![1; 12])
            .await;
        wait_for_maintenance(&cache);
        assert_eq!(cache.probe.scans.lock().unwrap().0, 2);
        assert_eq!(disk_bytes(&directory), 0);
    }

    #[tokio::test]
    async fn failed_atomic_rename_discards_partial_file_without_failing_image() {
        let dir = tempfile::tempdir().unwrap();
        let cache = DiskCache::new(dir.path().to_owned(), DISK_CACHE_MAX_AGE, u64::MAX);
        let (url, server) = local_image_server(png_bytes());
        fs::create_dir(dir.path().join(cache_key(&url))).unwrap();
        let limits = crate::media_scheduler::MediaScheduler::default().execution_limits();
        let (_, result) = limited_job(url, cache.clone()).run_limited(limits).await;
        server.join().unwrap();
        assert!(result.is_ok());
        wait_for_maintenance(&cache);
        assert_eq!(disk_bytes(dir.path()), 0);
        assert!(
            fs::read_dir(dir.path())
                .unwrap()
                .all(|entry| !is_staging_path(&entry.unwrap().path()))
        );
    }

    #[test]
    fn explicit_load_can_retry_a_sticky_failure() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = MediaCache::test_enabled(dir.path().to_owned());
        let url = "https://example.com/retry.jpg";
        cache.finish_load(url.into(), Err("permanent decode error".into()));

        assert!(!cache.prepare_image_load(url, false));
        assert!(cache.prepare_image_load(url, true));
        assert_eq!(cache.state_name(url), "missing");
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
        cache.mark_loading_url(&image.url);
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
