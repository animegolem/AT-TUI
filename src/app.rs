use std::{io, time::Duration};

use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::{
    api::BskyClient,
    media::{MediaCache, RequestedImageProtocol},
    model::{HomeFeedPrefs, feed_item_from_quote, thread_items, timeline_items},
    navigation::{NavigationStack, ViewKind, ViewState},
    ui,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Normal,
    Search { buffer: String },
}

pub struct App {
    pub client: BskyClient,
    pub nav: NavigationStack,
    pub media: MediaCache,
    pub home_feed_prefs: HomeFeedPrefs,
    pub status: String,
    pub input_mode: InputMode,
    pub should_quit: bool,
}

impl App {
    pub async fn bootstrap(mut client: BskyClient, media: MediaCache) -> Result<Self> {
        let (home_feed_prefs, pref_status) = match client.get_preferences().await {
            Ok(root) => (HomeFeedPrefs::from_preferences_response(&root), None),
            Err(error) => (
                HomeFeedPrefs::default(),
                Some(format!("Preferences unavailable: {error:#}")),
            ),
        };
        let root = client.get_timeline(None, 50).await?;
        let (items, cursor) = timeline_items(&root, &home_feed_prefs);
        let mut timeline = ViewState::new("Timeline", ViewKind::Timeline, items);
        timeline.cursor = cursor;
        let handle = client.session().handle.clone();
        let status = pref_status.unwrap_or_else(|| format!("Logged in as @{handle}"));
        Ok(Self {
            client,
            nav: NavigationStack::new(timeline),
            media,
            home_feed_prefs,
            status,
            input_mode: InputMode::Normal,
            should_quit: false,
        })
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
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

        if !self.should_quit {
            self.maybe_load_more().await?;
            let selected = self.nav.current().selected_item().cloned();
            if let Some(item) = selected {
                self.media.ensure_item(&item).await;
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

    async fn open_thread_for_selected(&mut self) -> Result<()> {
        let Some(selected) = self.nav.current().selected_item().cloned() else {
            self.status = "No selected post".into();
            return Ok(());
        };

        self.status = format!("Loading replies for @{}...", selected.author_handle);
        let root = self.client.get_post_thread(&selected.uri).await?;
        let items = thread_items(&root);
        if items.is_empty() {
            self.status = "No replies available".into();
            return Ok(());
        }

        let mut view = ViewState::new(
            format!("Thread @{}", selected.author_handle),
            ViewKind::Thread {
                root_uri: selected.uri.clone(),
            },
            items,
        );
        view.select_uri(&selected.uri);
        self.nav.push(view);
        self.status = "Thread loaded".into();
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

        self.status = format!("Loading quoted post @{}...", quote.author_handle);
        match self.client.get_post_thread(&quote.uri).await {
            Ok(root) => {
                let mut items = thread_items(&root);
                if items.is_empty() {
                    items.push(feed_item_from_quote(quote.clone(), 0));
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
                    vec![feed_item_from_quote(quote, 0)],
                ));
                self.status = format!("Quote preview only: {error:#}");
            }
        }
        Ok(())
    }

    async fn reload_current(&mut self) -> Result<()> {
        let kind = self.nav.current().kind.clone();
        match kind {
            ViewKind::Timeline => {
                let root = self.client.get_timeline(None, 50).await?;
                let (items, cursor) = timeline_items(&root, &self.home_feed_prefs);
                let current = self.nav.current_mut();
                current.items = items;
                current.cursor = cursor;
                current.selected = 0;
                current.scroll = 0;
                self.status = "Timeline refreshed".into();
            }
            ViewKind::Thread { root_uri } | ViewKind::Quote { uri: root_uri } => {
                let selected_uri = self
                    .nav
                    .current()
                    .selected_item()
                    .map(|item| item.uri.clone());
                let root = self.client.get_post_thread(&root_uri).await?;
                let items = thread_items(&root);
                let current = self.nav.current_mut();
                current.replace_items_preserving_uri(
                    items,
                    selected_uri.as_deref(),
                    Some(&root_uri),
                );
                self.status = "View refreshed".into();
            }
        }
        Ok(())
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
        let result = self.client.get_timeline(Some(&cursor), 50).await;
        let current = self.nav.current_mut();
        current.loading = false;

        match result {
            Ok(root) => {
                let (mut items, cursor) = timeline_items(&root, &self.home_feed_prefs);
                current.items.append(&mut items);
                current.cursor = cursor;
                self.status = "Loaded more timeline posts".into();
            }
            Err(error) => {
                current.error = Some(format!("{error:#}"));
                self.status = "Pagination failed".into();
            }
        }
        Ok(())
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
    if let Some(item) = app.nav.current().selected_item().cloned() {
        app.media.ensure_item(&item).await;
    }

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;
        if app.should_quit {
            break;
        }
        if event::poll(Duration::from_millis(150))?
            && let Event::Key(key) = event::read()?
        {
            app.handle_key(key).await?;
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
