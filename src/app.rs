use std::{
    future::Future,
    io,
    time::{Duration, Instant},
};

use anyhow::Result;
use chrono::{SecondsFormat, Utc};
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
        FeedItem, FeedSource, FeedSourceKind, HomeFeedPrefs, LinkRef, NotificationItem,
        NotificationTarget, PostRef, ProfileSummary, QuotePost, ViewItem, feed_item_from_quote,
        feed_sources_for_account, item_links, notification_items, profile_summary, thread_items,
        timeline_items,
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
    pub fn settings() -> Self {
        Self {
            section: MenuSection::Settings,
        }
    }

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MenuTabAction {
    MoveSection(isize),
    SwitchAccount(isize),
    SwitchFeed(isize),
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

fn menu_tab_action(section: MenuSection, reverse: bool) -> MenuTabAction {
    let delta = if reverse { -1 } else { 1 };
    match section {
        MenuSection::Accounts => MenuTabAction::SwitchAccount(delta),
        MenuSection::Feeds => MenuTabAction::SwitchFeed(delta),
        MenuSection::Keys | MenuSection::Settings => MenuTabAction::MoveSection(delta),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    MoveDown,
    MoveUp,
    OpenMenu,
    PreviewMedia,
    OpenLinks,
    PreviousFeed,
    NextFeed,
    LoadPending,
    ToggleLike,
    ToggleRepost,
    ComposePost,
    ComposeReply,
    ComposeQuote,
    JumpTop,
    JumpBottom,
    StartSearch,
    SearchNext,
    Back,
    Escape,
    OpenSelected,
    OpenQuote,
    OpenProfile,
    OpenNotifications,
    Reload,
}

pub fn normal_action_for_key(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Action::Quit),
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveUp),
        KeyCode::Char('?') => Some(Action::OpenMenu),
        KeyCode::Char(' ') => Some(Action::PreviewMedia),
        KeyCode::Char('u') => Some(Action::OpenLinks),
        KeyCode::Char('[') => Some(Action::PreviousFeed),
        KeyCode::Char(']') => Some(Action::NextFeed),
        KeyCode::Char('U') => Some(Action::LoadPending),
        KeyCode::Char('F') => Some(Action::ToggleLike),
        KeyCode::Char('R') => Some(Action::ToggleRepost),
        KeyCode::Char('p') => Some(Action::ComposePost),
        KeyCode::Char('c') => Some(Action::ComposeReply),
        KeyCode::Char('Q') => Some(Action::ComposeQuote),
        KeyCode::Char('g') => Some(Action::JumpTop),
        KeyCode::Char('G') => Some(Action::JumpBottom),
        KeyCode::Char('/') => Some(Action::StartSearch),
        KeyCode::Char('n') => Some(Action::SearchNext),
        KeyCode::Char('h') | KeyCode::Left => Some(Action::Back),
        KeyCode::Esc => Some(Action::Escape),
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => Some(Action::OpenSelected),
        KeyCode::Char('o') => Some(Action::OpenQuote),
        KeyCode::Char('P') => Some(Action::OpenProfile),
        KeyCode::Char('N') => Some(Action::OpenNotifications),
        KeyCode::Char('r') => Some(Action::Reload),
        _ => None,
    }
}

pub fn normal_key_help_lines() -> Vec<&'static str> {
    vec![
        "  j/k or arrows: move",
        "  l/Enter/Right: open selected · h/Left: back · Esc back/settings",
        "  P profile · N notifications · Space media · u links",
        "  F like · R repost · p post · c reply · Q quote",
        "  / search · n next · r reload · o quote · U load pending · q quit",
    ]
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
    ProfileLoaded {
        request_id: RequestId,
        actor: String,
        result: AppTaskResult<ProfileLoadData>,
    },
    NotificationsLoaded {
        request_id: RequestId,
        account_did: String,
        result: AppTaskResult<NotificationsLoadData>,
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
    NotificationCountLoaded {
        request_id: RequestId,
        account_did: String,
        result: AppTaskResult<u64>,
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

#[derive(Debug)]
struct ProfileLoadData {
    profile: ProfileSummary,
    items: Vec<FeedItem>,
    cursor: Option<String>,
}

#[derive(Debug)]
struct NotificationsLoadData {
    items: Vec<NotificationItem>,
    cursor: Option<String>,
    seen_updated: bool,
    seen_error: Option<String>,
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
    status_expires_at: Option<Instant>,
    pub input_mode: InputMode,
    pub overlay: Option<Overlay>,
    pub should_quit: bool,
    pub pending_new_items: Vec<FeedItem>,
    pub unread_notifications: u64,
    events_tx: UnboundedSender<AppEvent>,
    events_rx: UnboundedReceiver<AppEvent>,
    next_request_id: RequestId,
    pending_feed: Option<RequestId>,
    pending_pagination: Option<RequestId>,
    pending_refresh: Option<RequestId>,
    pending_notification_count: Option<RequestId>,
    pending_notifications: Option<RequestId>,
    pending_thread: Option<RequestId>,
    pending_profile: Option<RequestId>,
    pending_account: Option<RequestId>,
    pending_writes: usize,
    last_refresh: Instant,
    refresh_interval: Duration,
    last_notification_poll: Instant,
    notification_interval: Duration,
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
        let now = Instant::now();
        Ok(Self {
            client,
            nav: NavigationStack::new(timeline),
            media,
            accounts,
            feeds,
            active_feed: 0,
            home_feed_prefs,
            status,
            status_expires_at: Some(now + Duration::from_secs(2)),
            input_mode: InputMode::Normal,
            overlay: None,
            should_quit: false,
            pending_new_items: Vec::new(),
            unread_notifications: 0,
            events_tx,
            events_rx,
            next_request_id: 1,
            pending_feed: None,
            pending_pagination: None,
            pending_refresh: None,
            pending_notification_count: None,
            pending_notifications: None,
            pending_thread: None,
            pending_profile: None,
            pending_account: None,
            pending_writes: 0,
            last_refresh: now,
            refresh_interval: Duration::from_secs(60),
            last_notification_poll: now - Duration::from_secs(60),
            notification_interval: Duration::from_secs(60),
            last_video_frame: now,
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

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
        self.status_expires_at = Some(Instant::now() + Duration::from_secs(2));
    }

    pub fn visible_status(&self) -> Option<&str> {
        if self.status.is_empty() {
            return None;
        }
        if self
            .status_expires_at
            .is_some_and(|expires_at| Instant::now() <= expires_at)
        {
            Some(&self.status)
        } else {
            None
        }
    }

    #[cfg(test)]
    fn expire_status_for_test(&mut self) {
        self.status_expires_at = Some(Instant::now() - Duration::from_secs(1));
    }

    fn apply_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::ImageLoaded { url, result } => {
                self.media.finish_load(url.clone(), result);
                if matches!(self.media.state_name(&url), "ready") {
                    self.set_status("Image loaded");
                }
            }
            AppEvent::VideoLoaded {
                playlist_url,
                result,
            } => {
                self.media.finish_video_load(playlist_url.clone(), result);
                match self.media.video_state_name(&playlist_url) {
                    "ready" => self.set_status("Video frames ready"),
                    "failed" => self.set_status("Video decode failed"),
                    _ => {}
                }
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
            AppEvent::ProfileLoaded {
                request_id,
                actor,
                result,
            } => {
                if self.pending_profile == Some(request_id) {
                    self.pending_profile = None;
                    self.apply_profile_loaded(actor, result);
                }
            }
            AppEvent::NotificationsLoaded {
                request_id,
                account_did,
                result,
            } => {
                if self.pending_notifications == Some(request_id) {
                    self.pending_notifications = None;
                    if account_did == self.client.session().did {
                        self.apply_notifications_loaded(result);
                    }
                }
            }
            AppEvent::AccountLoaded { request_id, result } => {
                if self.pending_account == Some(request_id) {
                    self.pending_account = None;
                    self.apply_account_loaded(*result)?;
                }
            }
            AppEvent::LinkOpened { uri, result } => match result {
                Ok(()) => self.set_status(format!("Opened {uri}")),
                Err(error) => self.set_status(format!("Could not open link: {error}")),
            },
            AppEvent::WriteCompleted { result } => {
                self.pending_writes = self.pending_writes.saturating_sub(1);
                self.apply_write_result(result);
            }
            AppEvent::NotificationCountLoaded {
                request_id,
                account_did,
                result,
            } => {
                if self.pending_notification_count == Some(request_id) {
                    self.pending_notification_count = None;
                    if account_did == self.client.session().did {
                        match result {
                            Ok(count) => self.unread_notifications = count,
                            Err(_) => self.unread_notifications = 0,
                        }
                    }
                }
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
                        self.set_status("Search cancelled");
                    }
                    KeyCode::Enter => {
                        let query = buffer.clone();
                        self.input_mode = InputMode::Normal;
                        if self.nav.current_mut().search_next(&query) {
                            self.set_status(format!("Search: {query}"));
                        } else {
                            self.set_status(format!("No match: {query}"));
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
                KeyCode::Tab | KeyCode::BackTab => {
                    match menu_tab_action(state.section, matches!(key.code, KeyCode::BackTab)) {
                        MenuTabAction::MoveSection(delta) if delta > 0 => state.next(),
                        MenuTabAction::MoveSection(_) => state.previous(),
                        MenuTabAction::SwitchAccount(delta) => {
                            action = Action::SwitchAccount(delta);
                        }
                        MenuTabAction::SwitchFeed(delta) => {
                            action = Action::SwitchFeed(delta);
                        }
                    }
                }
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
                    self.set_status("No external media URL for selected item");
                }
            }
            Action::PlayVideo(video) => {
                if let Some(video) = video {
                    self.queue_video_load(&video);
                } else {
                    self.set_status("Selected media is not a video");
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
        let Some(action) = normal_action_for_key(key) else {
            return Ok(());
        };

        match action {
            Action::Quit => self.should_quit = true,
            Action::MoveDown => self.nav.current_mut().move_down(),
            Action::MoveUp => self.nav.current_mut().move_up(),
            Action::OpenMenu => self.overlay = Some(Overlay::Menu(MenuState::default())),
            Action::PreviewMedia => self.open_media_overlay_for_selected().await?,
            Action::OpenLinks => self.open_links_for_selected(),
            Action::PreviousFeed => self.switch_feed_delta(-1).await?,
            Action::NextFeed => self.switch_feed_delta(1).await?,
            Action::LoadPending => {
                self.nav.current_mut().jump_top();
                self.merge_pending_new_items(true);
            }
            Action::ToggleLike => self.toggle_like_selected(),
            Action::ToggleRepost => self.toggle_repost_selected(),
            Action::ComposePost => self.open_post_composer(),
            Action::ComposeReply => self.open_reply_composer(),
            Action::ComposeQuote => self.open_quote_composer(),
            Action::JumpTop => self.nav.current_mut().jump_top(),
            Action::JumpBottom => self.nav.current_mut().jump_bottom(),
            Action::StartSearch => {
                self.input_mode = InputMode::Search {
                    buffer: String::new(),
                };
                self.set_status("Search current view");
            }
            Action::SearchNext => {
                let query = self.nav.current().search_query.clone();
                if let Some(query) = query {
                    if self.nav.current_mut().search_next(&query) {
                        self.set_status(format!("Search: {query}"));
                    } else {
                        self.set_status(format!("No match: {query}"));
                    }
                }
            }
            Action::Back => {
                if self.nav.pop() {
                    self.set_status("Back");
                } else {
                    self.set_status("Already at timeline");
                }
            }
            Action::Escape => {
                if self.nav.pop() {
                    self.set_status("Back");
                } else {
                    self.overlay = Some(Overlay::Menu(MenuState::settings()));
                }
            }
            Action::OpenSelected => self.open_selected_detail().await?,
            Action::OpenQuote => self.open_quote_for_selected().await?,
            Action::OpenProfile => self.open_profile_for_selected(),
            Action::OpenNotifications => self.queue_notifications_load("Loading notifications"),
            Action::Reload => self.reload_current().await?,
        }
        Ok(())
    }

    async fn open_media_overlay_for_selected(&mut self) -> Result<()> {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.set_status("No selected post");
            return Ok(());
        };
        let media = preview_media(&item);
        if media.is_empty() {
            self.set_status("No media on selected post");
            return Ok(());
        }

        self.queue_media_thumbnail_loads(&media);
        self.overlay = Some(Overlay::Media(MediaOverlayState::new(media)));
        self.set_status("Media preview");
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
                "ready" => self.set_status("Video frames ready"),
                "loading" => self.set_status("Video decode already running"),
                "failed" => self.set_status("Video decode previously failed"),
                _ => {}
            }
            return;
        }

        self.media.mark_video_loading(video);
        let Some(job) = self.media.video_job(video) else {
            self.set_status("Video rendering disabled");
            return;
        };
        self.set_status("Decoding video frames");
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
            self.set_status("No selected post");
            return;
        };
        let links = item_links(item);
        match links.len() {
            0 => self.set_status("No links on selected post"),
            1 => self.open_link(links.into_iter().next().expect("one link")),
            _ => self.overlay = Some(Overlay::Links(LinkPickerState::new(links))),
        }
    }

    fn open_link(&mut self, link: LinkRef) {
        self.open_uri(link.uri);
    }

    fn open_uri(&mut self, uri: String) {
        self.set_status(format!("Opening {uri}"));
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
            self.set_status("No other feeds saved");
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
            self.set_status("No other accounts saved");
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
        self.set_status(status);
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
        self.set_status(action.loading_status());
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

    fn queue_profile_load(&mut self, actor: String, status: String) {
        let id = self.next_request_id();
        self.pending_profile = Some(id);
        self.set_status(status);
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = async {
                let root = client.get_profile(&actor).await?;
                let profile = profile_summary(&root);
                let feed_actor = if profile.did.is_empty() {
                    actor.as_str()
                } else {
                    profile.did.as_str()
                };
                let feed = client.get_author_feed(feed_actor, None, 50).await?;
                let (items, cursor) = timeline_items(&feed, &HomeFeedPrefs::default());
                Ok::<_, anyhow::Error>(ProfileLoadData {
                    profile,
                    items,
                    cursor,
                })
            }
            .await
            .map_err(|error| format!("{error:#}"));
            AppEvent::ProfileLoaded {
                request_id: id,
                actor,
                result,
            }
        });
    }

    fn queue_notifications_load(&mut self, status: impl Into<String>) {
        let id = self.next_request_id();
        self.pending_notifications = Some(id);
        self.set_status(status);
        let account_did = self.client.session().did.clone();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = async {
                let root = client.list_notifications(None, 50).await?;
                let (items, cursor, _) = notification_items(&root);
                let seen_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
                let seen_result = client.update_seen(&seen_at).await;
                Ok::<_, anyhow::Error>(NotificationsLoadData {
                    items,
                    cursor,
                    seen_updated: seen_result.is_ok(),
                    seen_error: seen_result.err().map(|error| format!("{error:#}")),
                })
            }
            .await
            .map_err(|error| format!("{error:#}"));
            AppEvent::NotificationsLoaded {
                request_id: id,
                account_did,
                result,
            }
        });
    }

    fn queue_account_switch(&mut self, account: AccountSession) {
        let id = self.next_request_id();
        self.pending_account = Some(id);
        self.set_status(format!("Switching to @{}", account.session.handle));
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
                self.set_status(format!("Loaded {}", source.label));
            }
            Err(error) => {
                self.nav.current_mut().error = Some(error);
                self.set_status("Feed load failed");
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
                current.append_posts(&mut items);
                current.cursor = cursor;
                self.set_status("Loaded more timeline posts");
            }
            Err(error) => {
                current.error = Some(error);
                self.set_status("Pagination failed");
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
                self.set_status(format!("Refresh check failed: {error}"));
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
            self.set_status(format!(
                "{} new posts pending",
                self.pending_new_items.len()
            ));
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
                self.set_status(if liked { "Liked post" } else { "Removed like" });
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
                self.set_status(if reposted {
                    "Reposted"
                } else {
                    "Removed repost"
                });
            }
            Ok(WriteResult::Posted { uri }) => {
                self.set_status(format!("Posted {uri}"));
                self.last_refresh = Instant::now() - self.refresh_interval;
            }
            Err(error) => self.set_status(format!("Write failed: {error}")),
        }
    }

    fn apply_thread_loaded(&mut self, action: ThreadAction, result: AppTaskResult<Vec<FeedItem>>) {
        match action {
            ThreadAction::OpenThread {
                selected_uri,
                title,
                kind,
            } => match result {
                Ok(items) if items.is_empty() => self.set_status("No replies available"),
                Ok(items) => {
                    let mut view = ViewState::new(title, kind, items);
                    view.select_uri(&selected_uri);
                    self.nav.push(view);
                    self.set_status("Thread loaded");
                }
                Err(error) => self.set_status(format!("Thread load failed: {error}")),
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
                    self.set_status("Quote loaded");
                }
                Err(error) => {
                    self.nav.push(ViewState::new(
                        format!("Quote @{}", quote.author_handle),
                        ViewKind::Quote {
                            uri: quote.uri.clone(),
                        },
                        vec![feed_item_from_quote(*quote, 0)],
                    ));
                    self.set_status(format!("Quote preview only: {error}"));
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
                    self.set_status("View refreshed");
                }
                Err(error) => self.set_status(format!("Refresh failed: {error}")),
            },
        }
    }

    fn apply_profile_loaded(&mut self, actor: String, result: AppTaskResult<ProfileLoadData>) {
        match result {
            Ok(data) => {
                let title = format!("Profile @{}", data.profile.handle);
                let mut view = ViewState::new(title, ViewKind::Profile { actor }, data.items);
                view.cursor = data.cursor;
                view.set_profile(data.profile);
                let view_actor = match &view.kind {
                    ViewKind::Profile { actor } => actor.clone(),
                    _ => String::new(),
                };
                let replaces_current = matches!(
                    &self.nav.current().kind,
                    ViewKind::Profile { actor } if actor == &view_actor
                );
                if replaces_current {
                    *self.nav.current_mut() = view;
                } else {
                    self.nav.push(view);
                }
                self.set_status("Profile loaded");
            }
            Err(error) => self.set_status(format!("Profile load failed: {error}")),
        }
    }

    fn apply_notifications_loaded(&mut self, result: AppTaskResult<NotificationsLoadData>) {
        match result {
            Ok(data) => {
                let rows = data
                    .items
                    .into_iter()
                    .map(|item| ViewItem::Notification(Box::new(item)))
                    .collect::<Vec<_>>();
                let mut view = ViewState::from_rows("Notifications", ViewKind::Notifications, rows);
                view.cursor = data.cursor;
                if matches!(self.nav.current().kind, ViewKind::Notifications) {
                    *self.nav.current_mut() = view;
                } else {
                    self.nav.push(view);
                }
                if data.seen_updated {
                    self.unread_notifications = 0;
                    self.last_notification_poll = Instant::now();
                    self.set_status("Notifications loaded");
                } else {
                    self.set_status(format!(
                        "Notifications loaded; seen update failed{}",
                        data.seen_error
                            .as_deref()
                            .map(|error| format!(": {error}"))
                            .unwrap_or_default()
                    ));
                }
            }
            Err(error) => self.set_status(format!("Notifications failed: {error}")),
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
                self.unread_notifications = 0;
                let mut view =
                    ViewState::new(self.feeds[0].label.clone(), ViewKind::Timeline, data.items);
                view.cursor = data.cursor;
                self.nav = NavigationStack::new(view);
                self.pending_new_items.clear();
                self.last_refresh = Instant::now();
                self.last_notification_poll = Instant::now() - self.notification_interval;
                self.set_status(format!("Switched to @{}", self.client.session().handle));
            }
            Err(error) => self.set_status(format!("Account switch failed: {error}")),
        }
        Ok(())
    }

    async fn open_selected_detail(&mut self) -> Result<()> {
        if let Some(selected) = self.nav.current().selected_item().cloned() {
            self.queue_thread_load(ThreadAction::OpenThread {
                selected_uri: selected.uri.clone(),
                title: format!("Thread @{}", selected.author_handle),
                kind: ViewKind::Thread {
                    root_uri: selected.uri.clone(),
                },
            });
            return Ok(());
        }

        if let Some(notification) = self.nav.current().selected_notification().cloned() {
            self.open_notification_target(notification);
            return Ok(());
        }

        self.set_status("No selectable item");
        Ok(())
    }

    fn open_notification_target(&mut self, notification: NotificationItem) {
        match notification.target {
            NotificationTarget::Post { uri } => {
                self.queue_thread_load(ThreadAction::OpenThread {
                    selected_uri: uri.clone(),
                    title: "Notification thread".into(),
                    kind: ViewKind::Thread { root_uri: uri },
                });
            }
            NotificationTarget::Profile { actor } => {
                self.queue_profile_load(actor, "Loading profile".into());
            }
            NotificationTarget::None => self.set_status("Notification has no openable target"),
        }
    }

    fn open_profile_for_selected(&mut self) {
        let Some((actor, label)) = self.selected_author_actor() else {
            self.set_status("No profile for selected item");
            return;
        };
        self.queue_profile_load(actor, format!("Loading profile @{label}"));
    }

    fn selected_author_actor(&self) -> Option<(String, String)> {
        if let Some(item) = self.nav.current().selected_item() {
            let actor = item
                .author_did
                .clone()
                .filter(|actor| !actor.is_empty())
                .unwrap_or_else(|| item.author_handle.clone());
            return Some((actor, item.author_handle.clone()));
        }

        let notification = self.nav.current().selected_notification()?;
        let actor = notification
            .author_did
            .clone()
            .filter(|actor| !actor.is_empty())
            .unwrap_or_else(|| notification.author_handle.clone());
        Some((actor, notification.author_handle.clone()))
    }

    async fn open_quote_for_selected(&mut self) -> Result<()> {
        let Some(quote) = self
            .nav
            .current()
            .selected_item()
            .and_then(|item| item.quote.clone())
        else {
            self.set_status("Selected post has no quote embed");
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
            self.set_status("Opened quoted post preview");
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
            ViewKind::Profile { actor } => {
                self.queue_profile_load(actor, "Refreshing profile".into());
            }
            ViewKind::Notifications => {
                self.queue_notifications_load("Refreshing notifications");
            }
        }
        Ok(())
    }

    fn open_post_composer(&mut self) {
        self.overlay = Some(Overlay::Composer(ComposerState {
            kind: ComposerKind::Post,
            buffer: String::new(),
        }));
        self.set_status("Compose post");
    }

    fn open_reply_composer(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.set_status("No selected post");
            return;
        };
        let Some(parent) = post_ref_from_item(&item) else {
            self.set_status("Selected post is missing a CID; cannot reply");
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
        self.set_status("Compose reply");
    }

    fn open_quote_composer(&mut self) {
        let Some(item) = self.nav.current().selected_item().cloned() else {
            self.set_status("No selected post");
            return;
        };
        let Some(quote) = post_ref_from_item(&item) else {
            self.set_status("Selected post is missing a CID; cannot quote");
            return;
        };
        self.overlay = Some(Overlay::Composer(ComposerState {
            kind: ComposerKind::Quote {
                quote,
                quote_handle: item.author_handle,
            },
            buffer: String::new(),
        }));
        self.set_status("Compose quote");
    }

    fn submit_composer(&mut self, state: ComposerState) {
        let text = state.buffer.trim().to_owned();
        if text.is_empty() {
            self.set_status("Post text is empty");
            return;
        }
        if text.chars().count() > 300 {
            self.set_status("Post is over 300 characters");
            return;
        }

        let (reply, quote) = match state.kind {
            ComposerKind::Post => (None, None),
            ComposerKind::Reply { root, parent, .. } => (Some((root, parent)), None),
            ComposerKind::Quote { quote, .. } => (None, Some(quote)),
        };

        self.overlay = None;
        self.pending_writes = self.pending_writes.saturating_add(1);
        self.set_status("Posting");
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
            self.set_status("No selected post");
            return;
        };
        let Some(subject) = post_ref_from_item(&item) else {
            self.set_status("Selected post is missing a CID; cannot like");
            return;
        };

        self.pending_writes = self.pending_writes.saturating_add(1);
        self.set_status(if item.viewer_like.is_some() {
            "Removing like"
        } else {
            "Liking post"
        });
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
            self.set_status("No selected post");
            return;
        };
        let Some(subject) = post_ref_from_item(&item) else {
            self.set_status("Selected post is missing a CID; cannot repost");
            return;
        };

        self.pending_writes = self.pending_writes.saturating_add(1);
        self.set_status(if item.viewer_repost.is_some() {
            "Removing repost"
        } else {
            "Reposting"
        });
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

    pub fn maybe_poll_notifications(&mut self) {
        if self.pending_notification_count.is_some()
            || self.last_notification_poll.elapsed() < self.notification_interval
        {
            return;
        }

        let id = self.next_request_id();
        self.pending_notification_count = Some(id);
        self.last_notification_poll = Instant::now();
        let account_did = self.client.session().did.clone();
        let mut client = self.client.clone();
        self.spawn_event(async move {
            let result = client
                .get_unread_notification_count()
                .await
                .map_err(|error| format!("{error:#}"));
            AppEvent::NotificationCountLoaded {
                request_id: id,
                account_did,
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
                self.set_status("No pending posts");
            }
            return;
        }

        let count = self.pending_new_items.len();
        let pending = std::mem::take(&mut self.pending_new_items);
        let current = self.nav.current_mut();
        current.prepend_posts(pending);
        current.selected = 0;
        current.scroll = 0;
        self.set_status(format!("Loaded {count} new posts"));
    }

    pub fn pending_new_count(&self) -> usize {
        self.pending_new_items.len()
    }

    pub fn has_pending_tasks(&self) -> bool {
        self.pending_feed.is_some()
            || self.pending_pagination.is_some()
            || self.pending_refresh.is_some()
            || self.pending_notification_count.is_some()
            || self.pending_notifications.is_some()
            || self.pending_thread.is_some()
            || self.pending_profile.is_some()
            || self.pending_account.is_some()
            || self.pending_writes > 0
    }

    pub fn pending_task_label(&self) -> Option<&'static str> {
        if self.pending_account.is_some() {
            Some("Switching account")
        } else if self.pending_feed.is_some() {
            Some("Loading feed")
        } else if self.pending_thread.is_some() {
            Some("Loading thread")
        } else if self.pending_profile.is_some() {
            Some("Loading profile")
        } else if self.pending_notifications.is_some() {
            Some("Loading notifications")
        } else if self.pending_pagination.is_some() {
            Some("Loading more")
        } else if self.pending_writes > 0 {
            Some("Writing")
        } else if self.pending_refresh.is_some() {
            Some("Refreshing")
        } else if self.pending_notification_count.is_some() {
            Some("Checking notifications")
        } else {
            None
        }
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
    current_items: &[ViewItem],
    pending_items: &[FeedItem],
    refreshed_items: Vec<FeedItem>,
) -> Vec<FeedItem> {
    let first_current_uri = current_items
        .iter()
        .find_map(ViewItem::as_post)
        .map(|item| item.uri.as_str());
    let mut known = current_items
        .iter()
        .filter_map(ViewItem::as_post)
        .map(|item| item.uri.clone())
        .chain(pending_items.iter().map(|item| item.uri.clone()))
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
        app.maybe_poll_notifications();
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
        model::{ImageRef, LinkSource, NotificationReason},
        ui,
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

    #[test]
    fn menu_tab_maps_to_section_account_and_feed_actions() {
        assert_eq!(
            menu_tab_action(MenuSection::Keys, false),
            MenuTabAction::MoveSection(1)
        );
        assert_eq!(
            menu_tab_action(MenuSection::Settings, true),
            MenuTabAction::MoveSection(-1)
        );
        assert_eq!(
            menu_tab_action(MenuSection::Accounts, false),
            MenuTabAction::SwitchAccount(1)
        );
        assert_eq!(
            menu_tab_action(MenuSection::Feeds, true),
            MenuTabAction::SwitchFeed(-1)
        );
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
        assert_eq!(app.nav.current().items[0].uri(), "original");
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
        let current = vec![ViewItem::from(item("old", "old"))];
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

    #[test]
    fn status_messages_expire() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        app.set_status("Loaded 1 new post");
        assert_eq!(app.visible_status(), Some("Loaded 1 new post"));
        app.expire_status_for_test();

        assert_eq!(app.visible_status(), None);
    }

    #[test]
    fn pending_task_label_survives_expired_status() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        app.set_status("Loading Following");
        app.expire_status_for_test();
        app.pending_feed = Some(1);

        assert_eq!(app.visible_status(), None);
        assert_eq!(app.pending_task_label(), Some("Loading feed"));
    }

    #[test]
    fn stale_notification_count_events_are_ignored() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        app.pending_notification_count = Some(2);

        app.apply_event(AppEvent::NotificationCountLoaded {
            request_id: 1,
            account_did: "did:plc:alice".into(),
            result: Ok(5),
        })
        .unwrap();

        assert_eq!(app.unread_notifications, 0);
        assert_eq!(app.pending_notification_count, Some(2));

        app.apply_event(AppEvent::NotificationCountLoaded {
            request_id: 2,
            account_did: "did:plc:other".into(),
            result: Ok(5),
        })
        .unwrap();

        assert_eq!(app.unread_notifications, 0);

        app.pending_notification_count = Some(3);
        app.apply_event(AppEvent::NotificationCountLoaded {
            request_id: 3,
            account_did: "did:plc:alice".into(),
            result: Ok(7),
        })
        .unwrap();

        assert_eq!(app.unread_notifications, 7);
    }

    #[test]
    fn normal_action_registry_maps_profile_notifications_and_existing_keys() {
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);

        assert_eq!(
            normal_action_for_key(key(KeyCode::Char('P'))),
            Some(Action::OpenProfile)
        );
        assert_eq!(
            normal_action_for_key(key(KeyCode::Char('N'))),
            Some(Action::OpenNotifications)
        );
        assert_eq!(
            normal_action_for_key(key(KeyCode::Char('l'))),
            Some(Action::OpenSelected)
        );
        assert_eq!(
            normal_action_for_key(key(KeyCode::Char('h'))),
            Some(Action::Back)
        );
        assert_eq!(
            normal_action_for_key(key(KeyCode::Esc)),
            Some(Action::Escape)
        );
        assert_eq!(
            normal_action_for_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Action::Quit)
        );
        assert!(
            normal_key_help_lines()
                .iter()
                .any(|line| line.contains("P profile") && line.contains("N notifications"))
        );
    }

    #[tokio::test]
    async fn escape_pops_when_nested_and_opens_settings_at_root() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        app.nav.push(ViewState::new(
            "Thread @alice.test",
            ViewKind::Thread {
                root_uri: "post".into(),
            },
            vec![item("post", "hello")],
        ));

        app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await
            .unwrap();

        assert_eq!(app.nav.depth(), 1);
        assert!(app.overlay.is_none());

        app.handle_normal_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
            .await
            .unwrap();

        assert!(matches!(
            app.overlay,
            Some(Overlay::Menu(MenuState {
                section: MenuSection::Settings
            }))
        ));
    }

    #[test]
    fn status_location_replaces_feed_when_nested() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        assert_eq!(ui::current_location_label(&app), "Following");

        app.nav.push(ViewState::new(
            "Thread @bob.test",
            ViewKind::Thread {
                root_uri: "post".into(),
            },
            vec![item("post", "hello")],
        ));

        assert_eq!(ui::current_location_label(&app), "Thread @bob.test");
        let line = line_text(&ui::status_left_line(&app, 200));
        assert!(line.contains("Thread @bob.test"));
        assert!(!line.contains("Following"));
    }

    #[test]
    fn status_counter_is_separate_right_segment_and_narrow_left_drops_status() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        app.set_status("Loaded a surprisingly long status message");

        let full = line_text(&ui::status_left_line(&app, 200));
        assert!(full.contains("Loaded a surprisingly long status message"));

        let narrow = line_text(&ui::status_left_line(&app, 1));
        assert!(!narrow.contains("Loaded a surprisingly long status message"));

        let right = ui::status_right_line(&app);
        assert_eq!(line_text(&right), " 1/1 ");
        assert_eq!(ui::line_width(&right), 5);
    }

    fn line_text(line: &ratatui::text::Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    }

    #[test]
    fn selected_author_actor_prefers_did_for_profile_navigation() {
        let mut item = item("post", "hello");
        item.author_did = Some("did:plc:alice".into());
        item.author_handle = "alice.test".into();
        let app = app_with_items(vec![item]);

        assert_eq!(
            app.selected_author_actor(),
            Some(("did:plc:alice".into(), "alice.test".into()))
        );
    }

    #[test]
    fn profile_loaded_pushes_profile_view_with_header_and_posts() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        app.apply_profile_loaded(
            "did:plc:bob".into(),
            Ok(ProfileLoadData {
                profile: ProfileSummary {
                    did: "did:plc:bob".into(),
                    handle: "bob.test".into(),
                    display_name: "Bob".into(),
                    description: Some("profile text".into()),
                    avatar_url: None,
                    banner_url: None,
                    followers_count: 1,
                    follows_count: 2,
                    posts_count: 3,
                },
                items: vec![item("bob-post", "bob text")],
                cursor: Some("next".into()),
            }),
        );

        assert!(matches!(
            app.nav.current().kind,
            ViewKind::Profile { ref actor } if actor == "did:plc:bob"
        ));
        assert_eq!(
            app.nav
                .current()
                .profile
                .as_ref()
                .map(|profile| profile.handle.as_str()),
            Some("bob.test")
        );
        assert_eq!(app.nav.current().items.len(), 1);
        assert_eq!(app.nav.current().cursor.as_deref(), Some("next"));
    }

    #[test]
    fn notifications_loaded_pushes_view_and_clears_unread_after_seen_update() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        app.unread_notifications = 4;

        app.apply_notifications_loaded(Ok(NotificationsLoadData {
            items: vec![notification(NotificationTarget::Post {
                uri: "post".into(),
            })],
            cursor: Some("next".into()),
            seen_updated: true,
            seen_error: None,
        }));

        assert!(matches!(app.nav.current().kind, ViewKind::Notifications));
        assert_eq!(app.unread_notifications, 0);
        assert_eq!(app.nav.current().cursor.as_deref(), Some("next"));
        assert!(matches!(
            app.nav.current().items.first(),
            Some(ViewItem::Notification(_))
        ));
    }

    #[test]
    fn notifications_seen_update_failure_leaves_unread_count() {
        let mut app = app_with_items(vec![item("post", "hello")]);
        app.unread_notifications = 4;

        app.apply_notifications_loaded(Ok(NotificationsLoadData {
            items: vec![notification(NotificationTarget::Profile {
                actor: "did:plc:bob".into(),
            })],
            cursor: None,
            seen_updated: false,
            seen_error: Some("network".into()),
        }));

        assert!(matches!(app.nav.current().kind, ViewKind::Notifications));
        assert_eq!(app.unread_notifications, 4);
        assert!(app.status.contains("seen update failed"));
    }

    #[tokio::test]
    async fn notification_targets_route_to_profile_or_thread_tasks() {
        let mut app = app_with_items(vec![item("post", "hello")]);

        app.open_notification_target(notification(NotificationTarget::Profile {
            actor: "did:plc:bob".into(),
        }));
        assert!(app.pending_profile.is_some());

        app.open_notification_target(notification(NotificationTarget::Post {
            uri: "at://did:plc:alice/app.bsky.feed.post/1".into(),
        }));
        assert!(app.pending_thread.is_some());
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
            status_expires_at: None,
            input_mode: InputMode::Normal,
            overlay: None,
            should_quit: false,
            pending_new_items: Vec::new(),
            unread_notifications: 0,
            events_tx,
            events_rx,
            next_request_id: 1,
            pending_feed: None,
            pending_pagination: None,
            pending_refresh: None,
            pending_notification_count: None,
            pending_notifications: None,
            pending_thread: None,
            pending_profile: None,
            pending_account: None,
            pending_writes: 0,
            last_refresh: Instant::now(),
            refresh_interval: Duration::from_secs(60),
            last_notification_poll: Instant::now(),
            notification_interval: Duration::from_secs(60),
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

    fn notification(target: NotificationTarget) -> NotificationItem {
        NotificationItem {
            uri: "at://did:plc:bob/app.bsky.notification/1".into(),
            cid: "cid".into(),
            author_did: Some("did:plc:bob".into()),
            author_name: "Bob".into(),
            author_handle: "bob.test".into(),
            reason: NotificationReason::Like,
            reason_subject: None,
            text: "notification text".into(),
            indexed_at: "2026-05-22T00:00:00Z".into(),
            is_read: false,
            target,
        }
    }
}
