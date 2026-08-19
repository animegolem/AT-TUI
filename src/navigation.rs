use ratatui::text::Line;

use crate::model::{FeedItem, NotificationItem, ProfileSummary, ViewItem};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewKind {
    Timeline,
    Thread { root_uri: String },
    Quote { uri: String },
    Profile { actor: String },
    Notifications,
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub title: String,
    pub kind: ViewKind,
    pub items: Vec<ViewItem>,
    pub profile: Option<ProfileSummary>,
    pub selected: usize,
    /// Zero-based rendered line at the top of the view buffer.
    pub scroll: usize,
    pub cursor: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
    pub search_query: Option<String>,
    pub layout_cache: LayoutCache,
}

#[derive(Debug, Clone, Default)]
pub struct LayoutCache {
    pub width: Option<usize>,
    pub items: Vec<CachedItemLines>,
    pub builds: usize,
}

impl LayoutCache {
    pub fn clear(&mut self) {
        self.width = None;
        self.items.clear();
    }
}

#[derive(Debug, Clone)]
pub struct CachedItemLines {
    pub selected: Vec<Line<'static>>,
    pub unselected: Vec<Line<'static>>,
}

impl ViewState {
    pub fn new(title: impl Into<String>, kind: ViewKind, items: Vec<FeedItem>) -> Self {
        Self::from_rows(title, kind, items.into_iter().map(ViewItem::from).collect())
    }

    pub fn from_rows(title: impl Into<String>, kind: ViewKind, items: Vec<ViewItem>) -> Self {
        Self {
            title: title.into(),
            kind,
            items,
            profile: None,
            selected: 0,
            scroll: 0,
            cursor: None,
            loading: false,
            error: None,
            search_query: None,
            layout_cache: LayoutCache::default(),
        }
    }

    pub fn selected_item(&self) -> Option<&FeedItem> {
        self.items.get(self.selected).and_then(ViewItem::as_post)
    }

    pub fn selected_notification(&self) -> Option<&NotificationItem> {
        match self.items.get(self.selected) {
            Some(ViewItem::Notification(item)) => Some(item.as_ref()),
            _ => None,
        }
    }

    pub fn set_profile(&mut self, profile: ProfileSummary) {
        self.profile = Some(profile);
    }

    pub fn append_posts(&mut self, posts: &mut Vec<FeedItem>) {
        self.items.extend(posts.drain(..).map(ViewItem::from));
        self.layout_cache.clear();
    }

    pub fn append_rows(&mut self, rows: Vec<ViewItem>) {
        self.items.extend(rows);
        self.layout_cache.clear();
    }

    pub fn prepend_posts(&mut self, posts: Vec<FeedItem>) {
        let mut rows = posts.into_iter().map(ViewItem::from).collect::<Vec<_>>();
        rows.append(&mut self.items);
        self.items = rows;
        self.layout_cache.clear();
    }

    pub fn select_uri(&mut self, uri: &str) -> bool {
        let Some(index) = self.items.iter().position(|item| item.uri() == uri) else {
            return false;
        };
        self.selected = index;
        if self.selected == 0 {
            self.scroll = 0;
        }
        true
    }

    pub fn replace_items_preserving_uri(
        &mut self,
        items: Vec<FeedItem>,
        preferred_uri: Option<&str>,
        fallback_uri: Option<&str>,
    ) {
        self.items = items.into_iter().map(ViewItem::from).collect();
        self.layout_cache.clear();

        let selected = preferred_uri
            .and_then(|uri| self.items.iter().position(|item| item.uri() == uri))
            .or_else(|| {
                fallback_uri.and_then(|uri| self.items.iter().position(|item| item.uri() == uri))
            })
            .unwrap_or(0);

        self.selected = selected.min(self.items.len().saturating_sub(1));
        self.scroll = 0;
    }

    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.items.len() - 1);
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        if self.selected == 0 {
            self.scroll = 0;
        }
    }

    pub fn move_by(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let last = (self.items.len() - 1) as isize;
        self.selected = (self.selected as isize + delta).clamp(0, last) as usize;
        if self.selected == 0 {
            self.scroll = 0;
        }
    }

    pub fn jump_top(&mut self) {
        self.selected = 0;
        self.scroll = 0;
    }

    pub fn jump_bottom(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = self.items.len() - 1;
    }

    pub fn search_next(&mut self, query: &str) -> bool {
        if query.trim().is_empty() || self.items.is_empty() {
            return false;
        }
        self.search_query = Some(query.to_owned());
        let query = query.to_lowercase();
        let start = self.selected.saturating_add(1);

        for index in start..self.items.len() {
            if item_matches(&self.items[index], &query) {
                self.selected = index;
                if self.selected == 0 {
                    self.scroll = 0;
                }
                return true;
            }
        }

        for index in 0..=self.selected.min(self.items.len() - 1) {
            if item_matches(&self.items[index], &query) {
                self.selected = index;
                if self.selected == 0 {
                    self.scroll = 0;
                }
                return true;
            }
        }

        false
    }
}

#[derive(Debug, Clone)]
pub struct NavigationStack {
    views: Vec<ViewState>,
}

impl NavigationStack {
    pub fn new(root: ViewState) -> Self {
        Self { views: vec![root] }
    }

    pub fn current(&self) -> &ViewState {
        self.views
            .last()
            .expect("navigation stack always has a root view")
    }

    pub fn current_mut(&mut self) -> &mut ViewState {
        self.views
            .last_mut()
            .expect("navigation stack always has a root view")
    }

    pub fn push(&mut self, view: ViewState) {
        self.views.push(view);
    }

    /// Swap out the root view (the timeline) without touching anything the
    /// user has pushed on top of it.
    pub fn replace_root(&mut self, view: ViewState) {
        self.views[0] = view;
    }

    pub fn root_mut(&mut self) -> &mut ViewState {
        self.views
            .first_mut()
            .expect("navigation stack always has a root view")
    }

    pub fn pop(&mut self) -> bool {
        if self.views.len() <= 1 {
            return false;
        }
        self.views.pop();
        true
    }

    pub fn depth(&self) -> usize {
        self.views.len()
    }

    pub fn breadcrumb(&self) -> String {
        self.views
            .iter()
            .map(|view| view.title.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }

    pub fn retain_items(&mut self, mut keep: impl FnMut(&ViewItem) -> bool) {
        for view in &mut self.views {
            let before = view.items.len();
            view.items.retain(&mut keep);
            if view.items.len() != before {
                view.layout_cache.clear();
                view.selected = view.selected.min(view.items.len().saturating_sub(1));
                view.scroll = 0;
            }
        }
    }

    pub fn for_each_item_mut(&mut self, mut f: impl FnMut(&mut FeedItem)) {
        for view in &mut self.views {
            for row in &mut view.items {
                if let Some(item) = row.as_post_mut() {
                    f(item);
                }
            }
            view.layout_cache.clear();
        }
    }

    pub fn for_each_notification_mut(&mut self, mut f: impl FnMut(&mut NotificationItem)) {
        for view in &mut self.views {
            for row in &mut view.items {
                if let Some(item) = row.as_notification_mut() {
                    f(item);
                }
            }
        }
    }

    pub fn for_each_profile_mut(&mut self, mut f: impl FnMut(&mut ProfileSummary)) {
        for view in &mut self.views {
            if let Some(profile) = &mut view.profile {
                f(profile);
            }
        }
    }
}

fn item_matches(item: &ViewItem, query: &str) -> bool {
    item.searchable_text().to_lowercase().contains(query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FeedItem;

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
            author_following_uri: None,
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

    #[test]
    fn move_by_clamps_at_both_ends() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![item("1", "one"), item("2", "two"), item("3", "three")],
        );

        view.move_by(10);
        assert_eq!(view.selected, 2);
        view.move_by(-1);
        assert_eq!(view.selected, 1);
        view.move_by(-10);
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn moves_up_and_down() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![item("1", "one"), item("2", "two")],
        );
        view.move_down();
        view.move_down();
        assert_eq!(view.selected, 1);
        view.move_up();
        assert_eq!(view.selected, 0);
    }

    #[test]
    fn replace_root_preserves_pushed_views() {
        let root = ViewState::new("Timeline", ViewKind::Timeline, vec![item("1", "one")]);
        let mut stack = NavigationStack::new(root);
        stack.push(ViewState::new(
            "Thread",
            ViewKind::Thread {
                root_uri: "1".into(),
            },
            vec![item("1", "one")],
        ));

        stack.replace_root(ViewState::new(
            "News",
            ViewKind::Timeline,
            vec![item("2", "two")],
        ));

        assert_eq!(stack.depth(), 2);
        assert_eq!(stack.current().title, "Thread");
        assert!(stack.pop());
        assert_eq!(stack.current().title, "News");
        assert_eq!(stack.current().items[0].uri(), "2");
    }

    #[test]
    fn stack_pop_restores_parent_cursor() {
        let mut root = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![item("1", "one"), item("2", "two")],
        );
        root.move_down();
        let mut stack = NavigationStack::new(root);
        stack.push(ViewState::new(
            "Thread",
            ViewKind::Thread {
                root_uri: "2".into(),
            },
            vec![item("2", "root")],
        ));
        assert_eq!(stack.depth(), 2);
        assert!(stack.pop());
        assert_eq!(stack.current().selected, 1);
    }

    #[test]
    fn selects_opened_thread_uri() {
        let mut view = ViewState::new(
            "Thread",
            ViewKind::Thread {
                root_uri: "selected".into(),
            },
            vec![
                item("root", "root"),
                item("parent", "parent"),
                item("selected", "selected"),
                item("reply", "reply"),
            ],
        );

        assert!(view.select_uri("selected"));
        assert_eq!(view.selected, 2);
    }

    #[test]
    fn replace_items_preserves_selected_uri() {
        let mut view = ViewState::new(
            "Thread",
            ViewKind::Thread {
                root_uri: "selected".into(),
            },
            vec![item("root", "root"), item("selected", "selected")],
        );
        view.select_uri("selected");

        view.replace_items_preserving_uri(
            vec![
                item("new-root", "new root"),
                item("selected", "selected"),
                item("reply", "reply"),
            ],
            Some("selected"),
            Some("new-root"),
        );

        assert_eq!(view.selected, 1);
    }

    #[test]
    fn replace_items_falls_back_to_root_uri() {
        let mut view = ViewState::new(
            "Thread",
            ViewKind::Thread {
                root_uri: "root".into(),
            },
            vec![item("old", "old")],
        );

        view.replace_items_preserving_uri(
            vec![item("root", "root"), item("reply", "reply")],
            Some("missing"),
            Some("root"),
        );

        assert_eq!(view.selected, 0);
    }

    #[test]
    fn search_wraps() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![item("1", "one"), item("2", "needle"), item("3", "three")],
        );
        view.selected = 2;
        assert!(view.search_next("needle"));
        assert_eq!(view.selected, 1);
    }
}
