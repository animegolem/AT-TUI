use std::{
    future::Future,
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use image::DynamicImage;
use ratatui::{Terminal, backend::CrosstermBackend};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    api::BskyClient,
    config::{AccountSession, Session},
    media::{
        MediaCache, PreviewImage, PreviewMedia, PreviewVideo, RequestedImageProtocol, preview_media,
    },
    model::{
        FeedItem, FeedSource, FeedSourceKind, HomeFeedPrefs, LinkRef, PostRef, QuotePost,
        feed_item_from_quote, feed_sources_for_account, item_links, thread_items, timeline_items,
    },
    navigation::{NavigationStack, ViewKind, ViewState},
    ui,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search { buffer: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Menu(MenuState),
    Media(MediaOverlayState),
    Links(LinkPickerState),
    Composer(ComposerState),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuState {
    pub section: MenuSection,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            section: MenuSection::Keys,
        }
    }
}

impl MenuState {
    pub fn next(&mut self) {
        self.section = self.section.next();
    }

    pub fn previous(&mut self) {
        self.section = self.section.previous();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuSection {
    Keys,
    Accounts,
    Feeds,
    Settings,
}

impl MenuSection {
    pub fn label(self) -> &'static str {
        match self {
            Self::Keys => "Keys",
            Self::Accounts => "Accounts",
            Self::Feeds => "Feeds",
            Self::Settings => "Settings",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Keys => Self::Accounts,
            Self::Accounts => Self::Feeds,
            Self::Feeds => Self::Settings,
            Self::Settings => Self::Keys,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::Keys => Self::Settings,
            Self::Accounts => Self::Keys,
            Self::Feeds => Self::Accounts,
            Self::Settings => Self::Feeds,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaOverlayState {
    pub media: Vec<PreviewMedia>,
    pub selected: usize,
    pub playing: bool,
}

impl MediaOverlayState {
    pub fn new(media: Vec<PreviewMedia>) -> Self {
        Self {
            media,
            selected: 0,
            playing: false,
        }
    }

    pub fn selected_media(&self) -> Option<&PreviewMedia> {
        self.media.get(self.selected)
    }

    pub fn next(&mut self) {
        if !self.media.is_empty() {
            self.selected = (self.selected + 1).min(self.media.len() - 1);
            self.playing = false;
        }
    }

    pub fn previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.playing = false;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposerState {
    pub kind: ComposerKind,
    pub buffer: String,
}

impl ComposerState {
    pub fn title(&self) -> &'static str {
        match self.kind {
            ComposerKind::Post => "New Post",
            ComposerKind::Reply { .. } => "Reply",
            ComposerKind::Quote { .. } => "Quote Post",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComposerKind {
    Post,
    Reply {
        root: PostRef,
        parent: PostRef,
        parent_handle: String,
    },
    Quote {
        quote: PostRef,
        quote_handle: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPickerState {
    pub links: Vec<LinkRef>,
    pub selected: usize,
}

impl LinkPickerState {
    pub fn new(links: Vec<LinkRef>) -> Self {
        Self { links, selected: 0 }
    }

    pub fn selected_link(&self) -> Option<&LinkRef> {
        self.links.get(self.selected)
    }

    pub fn next(&mut self) {
        if !self.links.is_empty() {
            self.selected = (self.selected + 1).min(self.links.len() - 1);
        }
    }

    pub fn previous(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
}

type RequestId = u64;

#[derive(Debug)]
enum AppEvent {
    ImageLoaded {
        url: String,
        result: std::result::Result<DynamicImage, String>,
    },
    VideoLoaded {
        playlist_url: String,
        result: std::result::Result<Vec<DynamicImage>, String>,
    },
    FeedLoaded {
        request_id: RequestId,
        source: FeedSource,
        result: AppTaskResult<(Vec<FeedItem>, Option<String>)>,
    },
    FeedRefreshLoaded {
        request_id: RequestId,
        source: FeedSource,
        result: AppTaskResult<Vec<FeedItem>>,
    },
    PageLoaded {
        request_id: RequestId,
        source: FeedSource,
        result: AppTaskResult<(Vec<FeedItem>, Option<String>)>,
    },
    ThreadLoaded {
        request_id: RequestId,
        action: ThreadAction,
        result: AppTaskResult<Vec<FeedItem>>,
    },
    AccountLoaded {
        request_id: RequestId,
        result: Box<AppTaskResult<AccountSwitchData>>,
    },
    LinkOpened {
        uri: String,
        result: std::result::Result<(), String>,
    },
    WriteCompleted {
        result: AppTaskResult<WriteResult>,
    },
}

type AppTaskResult<T> = std::result::Result<T, String>;

#[derive(Debug, Clone)]
enum ThreadAction {
    OpenThread {
        selected_uri: String,
        title: String,
        kind: ViewKind,
    },
    OpenQuote {
        quote: Box<QuotePost>,
    },
    Reload {
        root_uri: String,
        selected_uri: Option<String>,
    },
}

impl ThreadAction {
    fn root_uri(&self) -> &str {
        match self {
            Self::OpenThread { selected_uri, .. } => selected_uri,
            Self::OpenQuote { quote } => &quote.uri,
            Self::Reload { root_uri, .. } => root_uri,
        }
    }

    fn loading_status(&self) -> String {
        match self {
            Self::OpenThread { .. } => "Loading thread".into(),
            Self::OpenQuote { quote } => format!("Loading quoted post @{}...", quote.author_handle),
            Self::Reload { .. } => "Refreshing view".into(),
        }
    }
}

#[derive(Debug)]
struct AccountSwitchData {
    account: AccountSession,
    session: Session,
    home_feed_prefs: HomeFeedPrefs,
    feeds: Vec<FeedSource>,
    items: Vec<FeedItem>,
    cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WriteResult {
    Like {
        target_uri: String,
        liked: bool,
        record_uri: Option<String>,
    },
    Repost {
        target_uri: String,
        reposted: bool,
        record_uri: Option<String>,
    },
    Posted {
        uri: String,
    },
}

pub struct App {
    pub client: BskyClient,
    pub nav: NavigationStack,
    pub media: MediaCache,
    pub accounts: Vec<AccountSession>,
    pub feeds: Vec<FeedSource>,
    pub active_feed: usize,
    pub home_feed_prefs: HomeFeedPrefs,
    pub status: String,
    pub input_mode: InputMode,
    pub overlay: Option<Overlay>,
    pub should_quit: bool,
    pub pending_new_items: Vec<FeedItem>,
    events_tx: UnboundedSender<AppEvent>,
    events_rx: UnboundedReceiver<AppEvent>,
    next_request_id: RequestId,
    pending_feed: Option<RequestId>,
    pending_pagination: Option<RequestId>,
    pending_refresh: Option<RequestId>,
    pending_thread: Option<RequestId>,
    pending_account: Option<RequestId>,
    pending_writes: usize,
    last_refresh: Instant,
    refresh_interval: Duration,
    last_video_frame: Instant,
}

impl App {
    pub async fn bootstrap(mut client: BskyClient, media: MediaCache) -> Result<Self> {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let session_status = match client.refresh_session().await {
            Ok(()) => None,
            Err(error) => Some(format!("Session refresh failed: {error:#}")),
        };
        let accounts = client.store().list_accounts().unwrap_or_default();
        let (home_feed_prefs, feeds, pref_status) = match client.get_preferences().await {
            Ok(root) => (
                HomeFeedPrefs::from_preferences_response(&root),
                feed_sources_for_account(&root, &client.session().handle, &client.session().did),
                None,
            ),
            Err(error) => (
                HomeFeedPrefs::default(),
                vec![
                    FeedSource::home(),
                    FeedSource::author(&client.session().handle, &client.session().did),
                ],
                Some(format!("Preferences unavailable: {error:#}")),
            ),
        };
        let (items, cursor) =
            load_feed_page(&mut client, &feeds[0], &home_feed_prefs, None).await?;
        let mut timeline = ViewState::new(feeds[0].label.clone(), ViewKind::Timeline, items);
        timeline.cursor = cursor;
        let handle = client.session().handle.clone();
        let status = session_status
            .or(pref_status)
            .unwrap_or_else(|| format!("Logged in as @{handle}"));
        Ok(Self {
            client,
            nav: NavigationStack::new(timeline),
            media,
            accounts,
            feeds,
            active_feed: 0,
            home_feed_prefs,
            status,
            input_mode: InputMode::Normal,
            overlay: None,
            should_quit: false,
            pending_new_items: Vec::new(),
            events_tx,
            events_rx,
            next_request_id: 1,
            pending_feed: None,
            pending_pagination: None,
            pending_refresh: None,
            pending_thread: None,
            pending_account: None,
            pending_writes: 0,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(60),
            last_video_frame: Instant::now(),
        })
    }

    pub fn drain_events(&mut self) -> Result<()> {
        while let Ok(event) = self.events_rx.try_recv() {
            self.apply_event(event)?;
        }
        Ok(())
    }

    fn next_request_id(&mut self) -> RequestId {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        id
    }

    fn spawn_event<F>(&self, task: F)
    where
        F: Future<Output = AppEvent> + Send + 'static,
    {
        let tx = self.events_tx.clone();
        tokio::spawn(async move {
            let event = task.await;
            let _ = tx.send(event);
        });
    }

    fn apply_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::ImageLoaded { url, result } => {
                self.media.finish_load(url.clone(), result);
                if matches!(self.media.state_name(&url), "ready") {
                    self.status = "Image loaded".into();
                }
            }
            AppEvent::VideoLoaded {
                playlist_url,
                result,
            } => {
                self.media.finish_video_load(playlist_url.clone(), result);
                self.status = match self.media.video_state_name(&playlist_url) {
                    "ready" => "Video frames ready".into(),
                    "failed" => "Video decode failed".into(),
                    _ => self.status.clone(),
                };
            }
            AppEvent::FeedLoaded {
                request_id,
                source,
                result,
            } => {
                if self.pending_feed == Some(request_id) {
                    self.pending_feed = None;
                    self.apply_feed_loaded(source, result);
                }
            }
            AppEvent::FeedRefreshLoaded {
                request_id,
                source,
                result,
            } => {
                if self.pending_refresh == Some(request_id) {
                    self.pending_refresh = None;
                    self.apply_feed_refresh_loaded(source, result);
                }
            }
            AppEvent::PageLoaded {
                request_id,
                source,
                result,
            } => {
                if self.pending_pagination == Some(request_id) {
                    self.pending_pagination = None;
                    self.apply_page_loaded(source, result);
                }
            }
            AppEvent::ThreadLoaded {
                request_id,
                action,
                result,
            } => {
                if self.pending_thread == Some(request_id) {
                    self.pending_thread = None;
                    self.apply_thread_loaded(action, result);
                }
            }
            AppEvent::AccountLoaded { request_id, result } => {
                if self.pending_account == Some(request_id) {
                    self.pending_account = None;
                    self.apply_account_loaded(*result)?;
                }
            }
            AppEvent::LinkOpened { uri, result } => match result {
                Ok(()) => self.status = format!("Opened {uri}"),
                Err(error) => self.status = format!("Could not open link: {error}"),
            },
            AppEvent::WriteCompleted { result } => {
                self.pending_writes = self.pending_writes.saturating_sub(1);
                self.apply_write_result(result);
            }
        }
        Ok(())
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.overlay.is_some() {
            self.handle_overlay_key(key).await?;
        } else {
            match &mut self.input_mode {
                InputMode::Search { buffer } => match key.code {
                    KeyCode::Esc => {
                        self.input_mode = InputMode::Normal;
                        self.status = "Search cancelled".into();
                    }
                    KeyCode::Enter => {
                        let query = buffer.clone();
                        self.input_mode = InputMode::Normal;
                        if self.nav.current_mut().search_next(&query) {
                            self.status = format!("Search: {query}");
                        } else {
                            self.status = format!("No match: {query}");
                        }
                    }
                    KeyCode::Backspace => {
                        buffer.pop();
                    }
                    KeyCode::Char(c)
                        if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                    {
                        buffer.push(c);
                    }
                    KeyCode::Char(_) => {}
                    _ => {}
                },
                InputMode::Normal => self.handle_normal_key(key).await?,
            }
        }

        if !self.should_quit && self.overlay.is_none() {
            self.maybe_load_more().await?;
            if self.is_current_timeline_at_top() {
                self.merge_pending_new_items(false);
            }
        }

        Ok(())
    }

    async fn handle_overlay_key(&mut self, key: KeyEvent) -> Result<()> {
        enum Action {
            None,
            Close,
            SwitchAccount(isize),
            SwitchFeed(isize),
            OpenLink(Option<LinkRef>),
            OpenUri(Option<String>),
            PlayVideo(Option<PreviewVideo>),
            SubmitComposer(Option<ComposerState>),
        }

        let mut action = Action::None;
        match self.overlay.as_mut() {
            Some(Overlay::Menu(state)) => match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter | KeyCode::Char('q') => {
                    action = Action::Close;
                }
                KeyCode::Char('j') | KeyCode::Down => state.next(),
                KeyCode::Char('k') | KeyCode::Up => state.previous(),
                KeyCode::Char(' ') => match state.section {
                    MenuSection::Accounts => action = Action::SwitchAccount(1),
                    MenuSection::Feeds => action = Action::SwitchFeed(1),
                    MenuSection::Keys | MenuSection::Settings => {}
                },
                KeyCode::Char('a') => action = Action::SwitchAccount(1),
                KeyCode::Char('[') => action = Action::SwitchFeed(-1),
                KeyCode::Char(']') => action = Action::SwitchFeed(1),
                _ => {}
            },
            Some(Overlay::Media(state)) => match key.code {
                KeyCode::Esc | KeyCode::Char(' ') | KeyCode::Char('q') => {
                    action = Action::Close;
                }
                KeyCode::Char('h') | KeyCode::Left => state.previous(),
                KeyCode::Char('l') | KeyCode::Right => state.next(),
                KeyCode::Enter | KeyCode::Char('p') => {
                    let video = match state.selected_media() {
                        Some(PreviewMedia::Video(video)) => Some(video.clone()),
                        _ => None,
                    };
                    if video.is_some() {
                        state.playing = true;
                    }
                    action = Action::PlayVideo(video);
                }
                KeyCode::Char('u') => {
                    action = Action::OpenUri(match state.selected_media() {
                        Some(PreviewMedia::Video(video)) => Some(video.playlist_url.clone()),
                        _ => None,
                    });
                }
                _ => {}
            },
            Some(Overlay::Links(state)) => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {
                    action = Action::Close;
                }
                KeyCode::Char('j') | KeyCode::Down => state.next(),
                KeyCode::Char('k') | KeyCode::Up => state.previous(),
                KeyCode::Enter | KeyCode::Char('u') => {
                    action = Action::OpenLink(state.selected_link().cloned());
                }
                _ => {}
            },
            Some(Overlay::Composer(state)) => match key.code {
                KeyCode::Esc => action = Action::Close,
                KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    action = Action::SubmitComposer(Some(state.clone()));
                }
                KeyCode::Enter => state.buffer.push('\n'),
                KeyCode::Backspace => {
                    state.buffer.pop();
                }
                KeyCode::Char(c)
                    if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
                {
                    state.buffer.push(c);
                }
                _ => {}
            },
            None => {}
        }

        match action {
            Action::None => {}
            Action::Close => self.overlay = None,
            Action::SwitchAccount(delta) => self.switch_account_delta(delta).await?,
            Action::SwitchFeed(delta) => self.switch_feed_delta(delta).await?,
            Action::OpenLink(link) => {
                if let Some(link) = link {
                    self.open_link(link);
                }
            }
            Action::OpenUri(uri) => {
                if let Some(uri) = uri {
                    self.open_uri(uri);
                } else {
                    self.status = "No external media URL for selected item".into();
                }
            }
            Action::PlayVideo(video) => {
                if let Some(video) = video {
                    self.queue_video_load(&video);
                } else {
                    self.status = "Selected media is not a video".into();
                }
            }
            Action::SubmitComposer(state) => {
                if let Some(state) = state {
                    self.submit_composer(state);
                }
            }
        }
        Ok(())
    }

    async fn handle_normal_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true
            }
            KeyCode::Char('j') | KeyCode::Down => self.nav.current_mut().move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.nav.current_mut().move_up(),
            KeyCode::Char('?') => self.overlay = Some(Overlay::Menu(MenuState::default())),
            KeyCode::Char(' ') => self.open_media_overlay_for_selected().await?,
            KeyCode::Char('u') => self.open_links_for_selected(),
            KeyCode::Char('[') => self.switch_feed_delta(-1).await?,
            KeyCode::Char(']') => self.switch_feed_delta(1).await?,
            KeyCode::Char('U') => {
                self.nav.current_mut().jump_top();
                self.merge_pending_new_items(true);
            }
            KeyCode::Char('F') => self.toggle_like_selected(),
            KeyCode::Char('R') => self.toggle_repost_selected(),
            KeyCode::Char('p') => self.open_post_composer(),
            KeyCode::Char('c') => self.open_reply_composer(),
            KeyCode::Char('Q') => self.open_quote_composer(),
            KeyCode::Char('g') => self.nav.current_mut().jump_top(),
            KeyCode::Char('G') => self.nav.current_mut().jump_bottom(),
            KeyCode::Char('/') => {
                self.input_mode = InputMode::Search {
                    buffer: String::new(),
                };
                self.status = "Search current view".into();
            }
            KeyCode::Char('n') => {
                let query = self.nav.current().search_query.clone();
                if let Some(query) = query {
                    if self.nav.current_mut().search_next(&query) {
                        self.status = format!("Search: {query}");
                    } else {
                        self.status = format!("No match: {query}");
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::Esc => {
                if self.nav.pop() {
                    self.status = "Back".into();
                } else {
                    self.status = "Already at timeline".into();
                }
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                self.open_thread_for_selected().await?;
            }
            KeyCode::Char('o') => {
                self.open_quote_for_selected().await?;
            }
            KeyCode::Char('r') => {
                self.reload_current().await?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn open_media_overlay_for_selected(&mut self) -> Result<()> {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return Ok(());
        };
        let media = preview_media(&item);
        if media.is_empty() {
            self.status = "No media on selected post".into();
            return Ok(());
        }

        self.queue_media_thumbnail_loads(&media);
        self.overlay = Some(Overlay::Media(MediaOverlayState::new(media)));
        self.status = "Media preview".into();
        Ok(())
    }

    fn queue_media_thumbnail_loads(&mut self, media: &[PreviewMedia]) {
        let images = media
            .iter()
            .filter_map(|media| match media {
                PreviewMedia::Image(image) => Some(image.clone()),
                PreviewMedia::Video(video) => video.thumb_url.as_ref().map(|url| PreviewImage {
                    url: url.clone(),
                    alt: video.alt.clone(),
                    source: video.source,
                }),
            })
            .collect::<Vec<_>>();
        self.queue_image_loads(&images);
    }

    fn queue_image_loads(&mut self, images: &[PreviewImage]) {
        for image in images {
            if !self.media.should_load(image) {
                continue;
            }
            self.media.mark_loading(image);
            let Some(job) = self.media.load_job(image) else {
                continue;
            };
            self.spawn_event(async move {
                let (url, result) = job.run().await;
                AppEvent::ImageLoaded { url, result }
            });
        }
    }

    fn queue_video_load(&mut self, video: &PreviewVideo) {
        if !self.media.should_load_video(video) {
            match self.media.video_state_name(&video.playlist_url) {
                "ready" => self.status = "Video frames ready".into(),
                "loading" => self.status = "Video decode already running".into(),
                "failed" => self.status = "Video decode previously failed".into(),
                _ => {}
            }
            return;
        }

        self.media.mark_video_loading(video);
        let Some(job) = self.media.video_job(video) else {
            self.status = "Video rendering disabled".into();
            return;
        };
        self.status = "Decoding video frames".into();
        self.spawn_event(async move {
            let (playlist_url, result) = job.run().await;
            AppEvent::VideoLoaded {
                playlist_url,
                result,
            }
        });
    }

    fn open_links_for_selected(&mut self) {
        let Some(item) = self.nav.current().selected_item() else {
            self.status = "No selected post".into();
            return;
        };
        let links = item_links(item);
        match links.len() {
            0 => self.status = "No links on selected post".into(),
            1 => self.open_link(links.into_iter().next().expect("one link")),
            _ => self.overlay = Some(Overlay::Links(LinkPickerState::new(links))),
        }
    }

    fn open_link(&mut self, link: LinkRef) {
        self.open_uri(link.uri);
    }

    fn open_uri(&mut self, uri: String) {
        self.status = format!("Opening {uri}");
        self.spawn_event(async move {
            let opened_uri = uri.clone();
            #[cfg(test)]
            let result = Ok(());
            #[cfg(not(test))]
            let result = tokio::task::spawn_blocking(move || open::that(&uri))
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result.map_err(|error| error.to_string()));
            AppEvent::LinkOpened {
                uri: opened_uri,
                result,
            }
        });
    }

    async fn switch_feed_delta(&mut self, delta: isize) -> Result<()> {
        if self.feeds.len() <= 1 {
            self.status = "No other feeds saved".into();
            return Ok(());
        }

        let len = self.feeds.len() as isize;
        let next = (self.active_feed as isize + delta).rem_euclid(len) as usize;
        self.active_feed = next;
        self.queue_feed_load(
            self.feeds[next].clone(),
            format!("Loading {}", self.feeds[next].label),
        );
        Ok(())
    }

    async fn switch_account_delta(&mut self, delta: isize) -> Result<()> {
        if self.accounts.len() <= 1 {
            self.status = "No other accounts saved".into();
            return Ok(());
        }

        let current_did = self.client.session().did.clone();
        let current = self
            .accounts
            .iter()
            .position(|account| account.session.did == current_did)
            .unwrap_or(0);
        let len = self.accounts.len() as isize;
        let next = (current as isize + delta).rem_euclid(len) as usize;
        let account = self.accounts[next].clone();
        self.queue_account_switch(account);
        Ok(())
    }

    fn queue_feed_load(&mut self, source: FeedSource, status: String) {
        let id = self.next_request_id();
        self.pending_feed = Some(id);
        self.nav.current_mut().loading = true;
        self.status = status;
        let mut client = self.client.clone();
        let prefs = self.home_feed_prefs;
        self.spawn_event(async move {
            let result = load_feed_page(&mut client, &source, &prefs, None)
                .await
                .map_err(|error| format!("{error:#}"));
            AppEvent::FeedLoaded {
                request_id: id,
                source,
                result,
            }
        });
    }

    fn queue_thread_load(&mut self, action: ThreadAction) {
        let id = self.next_request_id();
        let uri = action.root_uri().to_owned();
        self.pending_thread = Some(id);
        self.status = action.loading_status();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = client
                .get_post_thread(&uri)
                .await
                .map(|root| thread_items(&root))
                .map_err(|error| format!("{error:#}"));
            AppEvent::ThreadLoaded {
                request_id: id,
                action,
                result,
            }
        });
    }

    fn queue_account_switch(&mut self, account: AccountSession) {
        let id = self.next_request_id();
        self.pending_account = Some(id);
        self.status = format!("Switching to @{}", account.session.handle);
        let store = self.client.store();
        self.spawn_event(async move {
            let mut client = BskyClient::new(account.session.clone(), store);
            let result = async {
                client.refresh_session().await?;
                let root = client.get_preferences().await?;
                let home_feed_prefs = HomeFeedPrefs::from_preferences_response(&root);
                let feeds = feed_sources_for_account(
                    &root,
                    &client.session().handle,
                    &client.session().did,
                );
                let source = feeds.first().cloned().unwrap_or_else(FeedSource::home);
                let (items, cursor) =
                    load_feed_page(&mut client, &source, &home_feed_prefs, None).await?;
                Ok::<_, anyhow::Error>(AccountSwitchData {
                    account,
                    session: client.session().clone(),
                    home_feed_prefs,
                    feeds,
                    items,
                    cursor,
                })
            }
            .await
            .map_err(|error| format!("{error:#}"));
            AppEvent::AccountLoaded {
                request_id: id,
                result: Box::new(result),
            }
        });
    }

    fn apply_feed_loaded(
        &mut self,
        source: FeedSource,
        result: AppTaskResult<(Vec<FeedItem>, Option<String>)>,
    ) {
        self.nav.current_mut().loading = false;
        match result {
            Ok((items, cursor)) => {
                let mut view = ViewState::new(source.label.clone(), ViewKind::Timeline, items);
                view.cursor = cursor;
                self.nav = NavigationStack::new(view);
                self.pending_new_items.clear();
                self.last_refresh = Instant::now();
                self.status = format!("Loaded {}", source.label);
            }
            Err(error) => {
                self.nav.current_mut().error = Some(error);
                self.status = "Feed load failed".into();
            }
        }
    }

    fn apply_page_loaded(
        &mut self,
        source: FeedSource,
        result: AppTaskResult<(Vec<FeedItem>, Option<String>)>,
    ) {
        let active_source = self.feeds.get(self.active_feed);
        let current = self.nav.current_mut();
        current.loading = false;
        if active_source != Some(&source) || !matches!(current.kind, ViewKind::Timeline) {
            return;
        }

        match result {
            Ok((mut items, cursor)) => {
                current.items.append(&mut items);
                current.layout_cache.clear();
                current.cursor = cursor;
                self.status = "Loaded more timeline posts".into();
            }
            Err(error) => {
                current.error = Some(error);
                self.status = "Pagination failed".into();
            }
        }
    }

    fn apply_feed_refresh_loaded(
        &mut self,
        source: FeedSource,
        result: AppTaskResult<Vec<FeedItem>>,
    ) {
        self.last_refresh = Instant::now();
        let Some(active_source) = self.feeds.get(self.active_feed) else {
            return;
        };
        let current = self.nav.current();
        if active_source != &source || !matches!(current.kind, ViewKind::Timeline) {
            return;
        }

        let refreshed = match result {
            Ok(items) => items,
            Err(error) => {
                self.status = format!("Refresh check failed: {error}");
                return;
            }
        };

        let new_items =
            new_items_before_current(current.items.as_slice(), &self.pending_new_items, refreshed);
        if new_items.is_empty() {
            return;
        }

        let mut merged = new_items;
        merged.append(&mut self.pending_new_items);
        self.pending_new_items = merged;

        if self.is_current_timeline_at_top() {
            self.merge_pending_new_items(false);
        } else {
            self.status = format!("{} new posts pending", self.pending_new_items.len());
        }
    }

    fn apply_write_result(&mut self, result: AppTaskResult<WriteResult>) {
        match result {
            Ok(WriteResult::Like {
                target_uri,
                liked,
                record_uri,
            }) => {
                self.nav.for_each_item_mut(|item| {
                    if item.uri == target_uri {
                        if liked {
                            if item.viewer_like.is_none() {
                                item.like_count = item.like_count.saturating_add(1);
                            }
                            item.viewer_like = record_uri.clone();
                        } else {
                            if item.viewer_like.is_some() {
                                item.like_count = item.like_count.saturating_sub(1);
                            }
                            item.viewer_like = None;
                        }
                    }
                });
                self.status = if liked { "Liked post" } else { "Removed like" }.into();
            }
            Ok(WriteResult::Repost {
                target_uri,
                reposted,
                record_uri,
            }) => {
                self.nav.for_each_item_mut(|item| {
                    if item.uri == target_uri {
                        if reposted {
                            if item.viewer_repost.is_none() {
                                item.repost_count = item.repost_count.saturating_add(1);
                            }
                            item.viewer_repost = record_uri.clone();
                        } else {
                            if item.viewer_repost.is_some() {
                                item.repost_count = item.repost_count.saturating_sub(1);
                            }
                            item.viewer_repost = None;
                        }
                    }
                });
                self.status = if reposted {
                    "Reposted"
                } else {
                    "Removed repost"
                }
                .into();
            }
            Ok(WriteResult::Posted { uri }) => {
                self.status = format!("Posted {uri}");
                self.last_refresh = Instant::now() - self.refresh_interval;
            }
            Err(error) => self.status = format!("Write failed: {error}"),
        }
    }

    fn apply_thread_loaded(&mut self, action: ThreadAction, result: AppTaskResult<Vec<FeedItem>>) {
        match action {
            ThreadAction::OpenThread {
                selected_uri,
                title,
                kind,
            } => match result {
                Ok(items) if items.is_empty() => self.status = "No replies available".into(),
                Ok(items) => {
                    let mut view = ViewState::new(title, kind, items);
                    view.select_uri(&selected_uri);
                    self.nav.push(view);
                    self.status = "Thread loaded".into();
                }
                Err(error) => self.status = format!("Thread load failed: {error}"),
            },
            ThreadAction::OpenQuote { quote } => match result {
                Ok(mut items) => {
                    if items.is_empty() {
                        items.push(feed_item_from_quote(quote.as_ref().clone(), 0));
                    }
                    let mut view = ViewState::new(
                        format!("Quote @{}", quote.author_handle),
                        ViewKind::Quote {
                            uri: quote.uri.clone(),
                        },
                        items,
                    );
                    view.select_uri(&quote.uri);
                    self.nav.push(view);
                    self.status = "Quote loaded".into();
                }
                Err(error) => {
                    self.nav.push(ViewState::new(
                        format!("Quote @{}", quote.author_handle),
                        ViewKind::Quote {
                            uri: quote.uri.clone(),
                        },
                        vec![feed_item_from_quote(*quote, 0)],
                    ));
                    self.status = format!("Quote preview only: {error}");
                }
            },
            ThreadAction::Reload {
                root_uri,
                selected_uri,
            } => match result {
                Ok(items) => {
                    let current = self.nav.current_mut();
                    current.replace_items_preserving_uri(
                        items,
                        selected_uri.as_deref(),
                        Some(&root_uri),
                    );
                    self.status = "View refreshed".into();
                }
                Err(error) => self.status = format!("Refresh failed: {error}"),
            },
        }
    }

    fn apply_account_loaded(&mut self, result: AppTaskResult<AccountSwitchData>) -> Result<()> {
        match result {
            Ok(data) => {
                let store = self.client.store();
                store.switch_account(&data.account.label)?;
                self.client = BskyClient::new(data.session, store.clone());
                self.accounts = store.list_accounts().unwrap_or_default();
                self.home_feed_prefs = data.home_feed_prefs;
                self.feeds = if data.feeds.is_empty() {
                    vec![FeedSource::home()]
                } else {
                    data.feeds
                };
                self.active_feed = 0;
                let mut view =
                    ViewState::new(self.feeds[0].label.clone(), ViewKind::Timeline, data.items);
                view.cursor = data.cursor;
                self.nav = NavigationStack::new(view);
                self.pending_new_items.clear();
                self.last_refresh = Instant::now();
                self.status = format!("Switched to @{}", self.client.session().handle);
            }
            Err(error) => self.status = format!("Account switch failed: {error}"),
        }
        Ok(())
    }

    async fn open_thread_for_selected(&mut self) -> Result<()> {
        let Some(selected) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return Ok(());
        };

        self.queue_thread_load(ThreadAction::OpenThread {
            selected_uri: selected.uri.clone(),
            title: format!("Thread @{}", selected.author_handle),
            kind: ViewKind::Thread {
                root_uri: selected.uri.clone(),
            },
        });
        Ok(())
    }

    async fn open_quote_for_selected(&mut self) -> Result<()> {
        let Some(quote) = self
            .nav
            .current()
            .selected_item()
            .and_then(|item| item.quote.clone())
        else {
            self.status = "Selected post has no quote embed".into();
            return Ok(());
        };

        if quote.uri.is_empty() {
            let item = feed_item_from_quote(quote, 0);
            self.nav.push(ViewState::new(
                "Quoted post",
                ViewKind::Quote {
                    uri: item.uri.clone(),
                },
                vec![item],
            ));
            self.status = "Opened quoted post preview".into();
            return Ok(());
        }

        self.queue_thread_load(ThreadAction::OpenQuote {
            quote: Box::new(quote),
        });
        Ok(())
    }

    async fn reload_current(&mut self) -> Result<()> {
        let kind = self.nav.current().kind.clone();
        match kind {
            ViewKind::Timeline => {
                let source = self.feeds[self.active_feed].clone();
                self.queue_feed_load(source, "Refreshing timeline".into());
            }
            ViewKind::Thread { root_uri } | ViewKind::Quote { uri: root_uri } => {
                let selected_uri = self
                    .nav
                    .current()
                    .selected_item()
                    .map(|item| item.uri.clone());
                self.queue_thread_load(ThreadAction::Reload {
                    root_uri,
                    selected_uri,
                });
            }
        }
        Ok(())
    }

    fn open_post_composer(&mut self) {
        self.overlay = Some(Overlay::Composer(ComposerState {
            kind: ComposerKind::Post,
            buffer: String::new(),
        }));
        self.status = "Compose post".into();
    }

    fn open_reply_composer(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return;
        };
        let Some(parent) = post_ref_from_item(&item) else {
            self.status = "Selected post is missing a CID; cannot reply".into();
            return;
        };
        let root = item.reply_root.clone().unwrap_or_else(|| parent.clone());
        self.overlay = Some(Overlay::Composer(ComposerState {
            kind: ComposerKind::Reply {
                root,
                parent,
                parent_handle: item.author_handle,
            },
            buffer: String::new(),
        }));
        self.status = "Compose reply".into();
    }

    fn open_quote_composer(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return;
        };
        let Some(quote) = post_ref_from_item(&item) else {
            self.status = "Selected post is missing a CID; cannot quote".into();
            return;
        };
        self.overlay = Some(Overlay::Composer(ComposerState {
            kind: ComposerKind::Quote {
                quote,
                quote_handle: item.author_handle,
            },
            buffer: String::new(),
        }));
        self.status = "Compose quote".into();
    }

    fn submit_composer(&mut self, state: ComposerState) {
        let text = state.buffer.trim().to_owned();
        if text.is_empty() {
            self.status = "Post text is empty".into();
            return;
        }
        if text.chars().count() > 300 {
            self.status = "Post is over 300 characters".into();
            return;
        }

        let (reply, quote) = match state.kind {
            ComposerKind::Post => (None, None),
            ComposerKind::Reply { root, parent, .. } => (Some((root, parent)), None),
            ComposerKind::Quote { quote, .. } => (None, Some(quote)),
        };

        self.overlay = None;
        self.pending_writes = self.pending_writes.saturating_add(1);
        self.status = "Posting".into();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = client
                .create_post(&text, reply, quote)
                .await
                .map(|record| WriteResult::Posted { uri: record.uri })
                .map_err(|error| format!("{error:#}"));
            AppEvent::WriteCompleted { result }
        });
    }

    fn toggle_like_selected(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return;
        };
        let Some(subject) = post_ref_from_item(&item) else {
            self.status = "Selected post is missing a CID; cannot like".into();
            return;
        };

        self.pending_writes = self.pending_writes.saturating_add(1);
        self.status = if item.viewer_like.is_some() {
            "Removing like".into()
        } else {
            "Liking post".into()
        };
        let target_uri = item.uri.clone();
        let existing = item.viewer_like.clone();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = async {
                if let Some(record_uri) = existing {
                    client.delete_record_uri(&record_uri).await?;
                    Ok(WriteResult::Like {
                        target_uri,
                        liked: false,
                        record_uri: None,
                    })
                } else {
                    let record = client.create_like(&subject).await?;
                    Ok(WriteResult::Like {
                        target_uri,
                        liked: true,
                        record_uri: Some(record.uri),
                    })
                }
            }
            .await
            .map_err(|error: anyhow::Error| format!("{error:#}"));
            AppEvent::WriteCompleted { result }
        });
    }

    fn toggle_repost_selected(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return;
        };
        let Some(subject) = post_ref_from_item(&item) else {
            self.status = "Selected post is missing a CID; cannot repost".into();
            return;
        };

        self.pending_writes = self.pending_writes.saturating_add(1);
        self.status = if item.viewer_repost.is_some() {
            "Removing repost".into()
        } else {
            "Reposting".into()
        };
        let target_uri = item.uri.clone();
        let existing = item.viewer_repost.clone();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = async {
                if let Some(record_uri) = existing {
                    client.delete_record_uri(&record_uri).await?;
                    Ok(WriteResult::Repost {
                        target_uri,
                        reposted: false,
                        record_uri: None,
                    })
                } else {
                    let record = client.create_repost(&subject).await?;
                    Ok(WriteResult::Repost {
                        target_uri,
                        reposted: true,
                        record_uri: Some(record.uri),
                    })
                }
            }
            .await
            .map_err(|error: anyhow::Error| format!("{error:#}"));
            AppEvent::WriteCompleted { result }
        });
    }

    async fn maybe_load_more(&mut self) -> Result<()> {
        let should_load = {
            let current = self.nav.current();
            matches!(current.kind, ViewKind::Timeline)
                && current.cursor.is_some()
                && !current.loading
                && current.selected.saturating_add(5) >= current.items.len()
        };

        if !should_load {
            return Ok(());
        }

        let cursor = self.nav.current().cursor.clone();
        let Some(cursor) = cursor else {
            return Ok(());
        };

        self.nav.current_mut().loading = true;
        let source = self.feeds[self.active_feed].clone();
        let id = self.next_request_id();
        self.pending_pagination = Some(id);
        let mut client = self.client.clone();
        let prefs = self.home_feed_prefs;
        self.spawn_event(async move {
            let result = load_feed_page(&mut client, &source, &prefs, Some(&cursor))
                .await
                .map_err(|error| format!("{error:#}"));
            AppEvent::PageLoaded {
                request_id: id,
                source,
                result,
            }
        });
        Ok(())
    }

    pub fn maybe_refresh_active_feed(&mut self) {
        if self.pending_refresh.is_some()
            || self.pending_feed.is_some()
            || self.last_refresh.elapsed() < self.refresh_interval
            || !matches!(self.nav.current().kind, ViewKind::Timeline)
        {
            return;
        }

        let source = self.feeds[self.active_feed].clone();
        let id = self.next_request_id();
        self.pending_refresh = Some(id);
        let mut client = self.client.clone();
        let prefs = self.home_feed_prefs;
        self.spawn_event(async move {
            let result = load_feed_page(&mut client, &source, &prefs, None)
                .await
                .map(|(items, _)| items)
                .map_err(|error| format!("{error:#}"));
            AppEvent::FeedRefreshLoaded {
                request_id: id,
                source,
                result,
            }
        });
    }

    fn is_current_timeline_at_top(&self) -> bool {
        let current = self.nav.current();
        matches!(current.kind, ViewKind::Timeline) && current.selected == 0 && current.scroll == 0
    }

    fn merge_pending_new_items(&mut self, explicit: bool) {
        if self.pending_new_items.is_empty()
            || !matches!(self.nav.current().kind, ViewKind::Timeline)
        {
            if explicit {
                self.status = "No pending posts".into();
            }
            return;
        }

        let count = self.pending_new_items.len();
        let mut pending = std::mem::take(&mut self.pending_new_items);
        let current = self.nav.current_mut();
        pending.append(&mut current.items);
        current.items = pending;
        current.selected = 0;
        current.scroll = 0;
        current.layout_cache.clear();
        self.status = format!("Loaded {count} new posts");
    }

    pub fn pending_new_count(&self) -> usize {
        self.pending_new_items.len()
    }

    pub fn has_pending_tasks(&self) -> bool {
        self.pending_feed.is_some()
            || self.pending_pagination.is_some()
            || self.pending_refresh.is_some()
            || self.pending_thread.is_some()
            || self.pending_account.is_some()
            || self.pending_writes > 0
    }

    pub fn current_position_label(&self) -> String {
        let current = self.nav.current();
        if current.items.is_empty() {
            "0/0".into()
        } else {
            format!("{}/{}", current.selected + 1, current.items.len())
        }
    }

    pub fn advance_video_frame(&mut self) {
        if self.last_video_frame.elapsed() < Duration::from_millis(125) {
            return;
        }
        let Some(Overlay::Media(state)) = self.overlay.as_ref() else {
            return;
        };
        if !state.playing {
            return;
        }
        let Some(PreviewMedia::Video(video)) = state.selected_media() else {
            return;
        };
        let playlist_url = video.playlist_url.clone();
        self.media.advance_video(&playlist_url);
        self.last_video_frame = Instant::now();
    }
}

fn post_ref_from_item(item: &FeedItem) -> Option<PostRef> {
    Some(PostRef {
        uri: item.uri.clone(),
        cid: item.cid.clone()?,
    })
}

fn new_items_before_current(
    current_items: &[FeedItem],
    pending_items: &[FeedItem],
    refreshed_items: Vec<FeedItem>,
) -> Vec<FeedItem> {
    let first_current_uri = current_items.first().map(|item| item.uri.as_str());
    let mut known = current_items
        .iter()
        .chain(pending_items.iter())
        .map(|item| item.uri.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut new_items = Vec::new();
    for item in refreshed_items {
        if Some(item.uri.as_str()) == first_current_uri {
            break;
        }
        if known.insert(item.uri.clone()) {
            new_items.push(item);
        }
    }
    new_items
}

async fn load_feed_page(
    client: &mut BskyClient,
    source: &FeedSource,
    home_prefs: &HomeFeedPrefs,
    cursor: Option<&str>,
) -> Result<(Vec<crate::model::FeedItem>, Option<String>)> {
    match &source.kind {
        FeedSourceKind::Home => {
            let root = client.get_timeline(cursor, 50).await?;
            Ok(timeline_items(&root, home_prefs))
        }
        FeedSourceKind::Author { did, .. } => {
            let root = client.get_author_feed(did, cursor, 50).await?;
            Ok(timeline_items(&root, &HomeFeedPrefs::default()))
        }
        FeedSourceKind::Generator { uri } => {
            let root = client.get_feed(uri, cursor, 50).await?;
            Ok(timeline_items(&root, &HomeFeedPrefs::default()))
        }
    }
}

pub async fn run_tui(
    client: BskyClient,
    requested_protocol: RequestedImageProtocol,
    no_images: bool,
) -> Result<()> {
    enable_raw_mode()?;
    let _session = TerminalSession;

    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let media = if no_images {
        MediaCache::disabled()
    } else {
        MediaCache::new(true, requested_protocol)?
    };

    let mut app = App::bootstrap(client, media).await?;

    loop {
        app.drain_events()?;
        app.maybe_refresh_active_feed();
        app.advance_video_frame();
        terminal.draw(|frame| ui::render(frame, &mut app))?;
        if app.should_quit {
            break;
        }
        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key).await?;
            app.drain_events()?;
        }
    }

    terminal.show_cursor()?;
    Ok(())
}

struct TerminalSession;

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let mut stdout = io::stdout();
        let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::SessionStore,
        media::PreviewImageSource,
        model::{ImageRef, LinkSource},
    };

    fn image(url: &str) -> PreviewImage {
        PreviewImage {
            url: url.into(),
            alt: None,
            source: PreviewImageSource::Post,
        }
    }

    #[test]
    fn media_overlay_navigation_stays_in_bounds() {
        let mut state = MediaOverlayState::new(vec![
            PreviewMedia::Image(image("one")),
            PreviewMedia::Image(image("two")),
        ]);

        assert!(
            matches!(state.selected_media(), Some(PreviewMedia::Image(image)) if image.url == "one")
        );
        state.previous();
        assert_eq!(state.selected, 0);
        state.next();
        state.next();
        assert_eq!(state.selected, 1);
        assert!(
            matches!(state.selected_media(), Some(PreviewMedia::Image(image)) if image.url == "two")
        );
    }

    #[test]
    fn menu_state_cycles_sections() {
        let mut state = MenuState::default();

        assert_eq!(state.section, MenuSection::Keys);
        state.next();
        assert_eq!(state.section, MenuSection::Accounts);
        state.next();
        assert_eq!(state.section, MenuSection::Feeds);
        state.previous();
        assert_eq!(state.section, MenuSection::Accounts);
    }

    #[tokio::test]
    async fn image_overlay_opens_before_image_load_finishes() {
        let mut item = item("post", "hello");
        item.images.push(ImageRef {
            thumb_url: "https://example.com/thumb.jpg".into(),
            fullsize_url: Some("https://example.com/full.jpg".into()),
            alt: None,
        });
        let mut app = app_with_items(vec![item]);

        app.open_media_overlay_for_selected().await.unwrap();

        assert!(matches!(app.overlay, Some(Overlay::Media(_))));
    }

    #[test]
    fn stale_pagination_event_is_ignored() {
        let mut app = app_with_items(vec![item("original", "hello")]);
        app.pending_pagination = Some(2);

        app.apply_event(AppEvent::PageLoaded {
            request_id: 1,
            source: FeedSource::home(),
            result: Ok((vec![item("stale", "stale")], None)),
        })
        .unwrap();

        assert_eq!(app.nav.current().items.len(), 1);
        assert_eq!(app.nav.current().items[0].uri, "original");
    }

    #[test]
    fn link_picker_opens_for_multiple_links() {
        let mut item = item("post", "hello");
        item.links = vec![
            LinkRef {
                uri: "https://one.test".into(),
                label: "one".into(),
                source: LinkSource::Text,
            },
            LinkRef {
                uri: "https://two.test".into(),
                label: "two".into(),
                source: LinkSource::Text,
            },
        ];
        let mut app = app_with_items(vec![item]);

        app.open_links_for_selected();

        assert!(matches!(app.overlay, Some(Overlay::Links(_))));
    }

    #[test]
    fn link_picker_reports_no_links() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        app.open_links_for_selected();

        assert_eq!(app.status, "No links on selected post");
    }

    #[test]
    fn pending_refresh_items_merge_only_when_requested() {
        let current = vec![item("old", "old")];
        let refreshed = vec![item("new", "new"), item("old", "old")];

        let new_items = new_items_before_current(&current, &[], refreshed);

        assert_eq!(
            new_items
                .iter()
                .map(|item| item.uri.as_str())
                .collect::<Vec<_>>(),
            vec!["new"]
        );
    }

    #[test]
    fn composer_rejects_empty_post_without_closing() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        let state = ComposerState {
            kind: ComposerKind::Post,
            buffer: "   ".into(),
        };

        app.submit_composer(state);

        assert_eq!(app.status, "Post text is empty");
        assert_eq!(app.pending_writes, 0);
    }

    fn app_with_items(items: Vec<FeedItem>) -> App {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::from_path(dir.path().join("accounts.json"));
        let session = Session {
            service: "https://bsky.social".into(),
            handle: "alice.test".into(),
            did: "did:plc:alice".into(),
            access_jwt: "access".into(),
            refresh_jwt: "refresh".into(),
        };
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        App {
            client: BskyClient::new(session, store),
            nav: NavigationStack::new(ViewState::new("Timeline", ViewKind::Timeline, items)),
            media: MediaCache::disabled(),
            accounts: Vec::new(),
            feeds: vec![FeedSource::home()],
            active_feed: 0,
            home_feed_prefs: HomeFeedPrefs::default(),
            status: String::new(),
            input_mode: InputMode::Normal,
            overlay: None,
            should_quit: false,
            pending_new_items: Vec::new(),
            events_tx,
            events_rx,
            next_request_id: 1,
            pending_feed: None,
            pending_pagination: None,
            pending_refresh: None,
            pending_thread: None,
            pending_account: None,
            pending_writes: 0,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(60),
            last_video_frame: Instant::now(),
        }
    }

    fn item(uri: &str, text: &str) -> FeedItem {
        FeedItem {
            uri: uri.into(),
            cid: None,
            viewer_like: None,
            viewer_repost: None,
            author_did: None,
            author_name: "Alice".into(),
            author_handle: "alice.test".into(),
            author_following: None,
            avatar_url: None,
            text: text.into(),
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
