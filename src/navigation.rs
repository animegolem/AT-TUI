use ratatui::text::Line;

use crate::model::FeedItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewKind {
    Timeline,
    Thread { root_uri: String },
    Quote { uri: String },
}

#[derive(Debug, Clone)]
pub struct ViewState {
    pub title: String,
    pub kind: ViewKind,
    pub items: Vec<FeedItem>,
    pub selected: usize,
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
        Self {
            title: title.into(),
            kind,
            items,
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
        self.items.get(self.selected)
    }

    pub fn select_uri(&mut self, uri: &str) -> bool {
        let Some(index) = self.items.iter().position(|item| item.uri == uri) else {
            return false;
        };
        self.selected = index;
        if self.scroll > self.selected {
            self.scroll = self.selected;
        }
        true
    }

    pub fn replace_items_preserving_uri(
        &mut self,
        items: Vec<FeedItem>,
        preferred_uri: Option<&str>,
        fallback_uri: Option<&str>,
    ) {
        self.items = items;
        self.layout_cache.clear();

        let selected = preferred_uri
            .and_then(|uri| self.items.iter().position(|item| item.uri == uri))
            .or_else(|| {
                fallback_uri.and_then(|uri| self.items.iter().position(|item| item.uri == uri))
            })
            .unwrap_or(0);

        self.selected = selected.min(self.items.len().saturating_sub(1));
        self.scroll = self.scroll.min(self.selected);
    }

    pub fn move_down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1).min(self.items.len() - 1);
        self.ensure_cursor_visible();
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.ensure_cursor_visible();
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
        self.ensure_cursor_visible();
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
                self.ensure_cursor_visible();
                return true;
            }
        }

        for index in 0..=self.selected.min(self.items.len() - 1) {
            if item_matches(&self.items[index], &query) {
                self.selected = index;
                self.ensure_cursor_visible();
                return true;
            }
        }

        false
    }

    pub fn ensure_scroll_for_height(&mut self, height: usize) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
        if height == 0 {
            return;
        }
        while self.selected.saturating_sub(self.scroll) >= height {
            self.scroll += 1;
        }
    }

    fn ensure_cursor_visible(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        }
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

    pub fn for_each_item_mut(&mut self, mut f: impl FnMut(&mut FeedItem)) {
        for view in &mut self.views {
            for item in &mut view.items {
                f(item);
            }
            view.layout_cache.clear();
        }
    }
}

fn item_matches(item: &FeedItem, query: &str) -> bool {
    item.text.to_lowercase().contains(query)
        || item.author_handle.to_lowercase().contains(query)
        || item.author_name.to_lowercase().contains(query)
        || item
            .quote
            .as_ref()
            .is_some_and(|quote| quote.text.to_lowercase().contains(query))
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
