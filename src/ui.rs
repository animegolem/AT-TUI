use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::{App, ComposerKind, ComposerState, InputMode, MenuSection, Overlay},
    media::{PreviewImage, PreviewMedia},
    model::{
        ExternalRef, FeedItem, FeedReason, ImageRef, QuotePost, ReplyContext, ReplyParentStatus,
        compact_time,
    },
    navigation::{CachedItemLines, ViewState},
};

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    render_body(frame, chunks[0], app);
    render_status(frame, chunks[1], app);
    render_overlay(frame, area, app);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    render_feed(frame, area, app);
}

fn render_feed(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let view = app.nav.current_mut();
    let title = if view.loading {
        format!("{} [loading]", view.title)
    } else {
        view.title.clone()
    };
    let block = rounded_block().title(title);
    let inner_width = area.width.saturating_sub(4).max(12) as usize;
    let available_lines = area.height.saturating_sub(2) as usize;
    let mut lines = visible_feed_lines(view, inner_width, available_lines);

    if let Some(error) = &view.error {
        lines.push(Line::from(vec![Span::styled(
            format!("Error: {error}"),
            Style::default().fg(Color::Red),
        )]));
    }

    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
}

fn visible_feed_lines(
    view: &mut ViewState,
    width: usize,
    available_lines: usize,
) -> Vec<Line<'static>> {
    ensure_layout_cache(view, width);
    ensure_selected_rendered(view, width, available_lines);

    if view.items.is_empty() {
        return vec![Line::from("No posts in this view.")];
    }

    let mut lines = Vec::new();
    let mut used = 0usize;
    for index in view.scroll..view.items.len() {
        if used >= available_lines {
            break;
        }

        let selected = index == view.selected;
        let item_lines = cached_item_lines(view, index, selected);

        if used + item_lines.len() > available_lines {
            let remaining = available_lines.saturating_sub(used);
            lines.extend(item_lines.into_iter().take(remaining));
            break;
        }

        used += item_lines.len();
        lines.extend(item_lines);
    }

    lines
}

fn ensure_layout_cache(view: &mut ViewState, width: usize) {
    let needs_rebuild =
        view.layout_cache.width != Some(width) || view.layout_cache.items.len() != view.items.len();
    if !needs_rebuild {
        return;
    }

    view.layout_cache.width = Some(width);
    view.layout_cache.items = view
        .items
        .iter()
        .map(|item| CachedItemLines {
            selected: render_item_lines(item, true, width),
            unselected: render_item_lines(item, false, width),
        })
        .collect();
    view.layout_cache.builds += 1;
}

fn cached_item_lines(view: &ViewState, index: usize, selected: bool) -> Vec<Line<'static>> {
    let Some(item) = view.layout_cache.items.get(index) else {
        return Vec::new();
    };
    if selected {
        item.selected.clone()
    } else {
        item.unselected.clone()
    }
}

fn ensure_selected_rendered(view: &mut ViewState, width: usize, available_lines: usize) {
    if view.items.is_empty() {
        view.selected = 0;
        view.scroll = 0;
        return;
    }

    let last_index = view.items.len() - 1;
    view.selected = view.selected.min(last_index);
    view.scroll = view.scroll.min(last_index);

    if view.selected < view.scroll {
        view.scroll = view.selected;
        return;
    }

    if available_lines == 0 {
        return;
    }

    while view.scroll < view.selected
        && rendered_height(view, view.scroll, view.selected, width) > available_lines
    {
        view.scroll += 1;
    }
}

fn rendered_height(view: &ViewState, start: usize, end: usize, width: usize) -> usize {
    if view.layout_cache.width != Some(width) {
        return view.items[start..=end]
            .iter()
            .map(|item| render_item_lines(item, false, width).len())
            .sum();
    }

    view.layout_cache.items[start..=end]
        .iter()
        .map(|item| item.unselected.len())
        .sum()
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App) {
    frame.render_widget(Paragraph::new(status_line(app)), area);
}

fn render_overlay(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let Some(overlay) = app.overlay.clone() else {
        return;
    };

    match overlay {
        Overlay::Menu(state) => render_menu_overlay(frame, area, app, state.section),
        Overlay::Media(state) => {
            if let Some(media) = state.selected_media().cloned() {
                render_media_overlay(frame, area, app, &media, state.selected, state.media.len());
            }
        }
        Overlay::Links(state) => render_link_overlay(frame, area, state.links, state.selected),
        Overlay::Composer(state) => render_composer_overlay(frame, area, state),
    }
}

fn render_menu_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, selected: MenuSection) {
    let area = centered_rect(86, 82, area);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    lines.push(section_line(MenuSection::Keys, selected));
    lines.push(Line::from("  j/k or arrows: move"));
    lines.push(Line::from("  l/Enter/Right: open thread"));
    lines.push(Line::from("  h/Esc/Left: back"));
    lines.push(Line::from("  Space: preview media"));
    lines.push(Line::from(
        "  u links, F like, R repost, p post, c reply, Q quote",
    ));
    lines.push(Line::from(
        "  / search, n next, r reload, o open quote, U load pending, q quit",
    ));
    lines.push(Line::from(""));

    lines.push(section_line(MenuSection::Accounts, selected));
    lines.push(Line::from(format!(
        "  active: @{}",
        app.client.session().handle
    )));
    for account in app.accounts.iter().take(6) {
        let marker = if account.session.did == app.client.session().did {
            "*"
        } else {
            " "
        };
        lines.push(Line::from(format!(
            "  {marker} {} @{}",
            account.label, account.session.handle
        )));
    }
    lines.push(Line::from("  a or Space on this section: next account"));
    lines.push(Line::from(""));

    lines.push(section_line(MenuSection::Feeds, selected));
    for (index, feed) in app.feeds.iter().take(8).enumerate() {
        let marker = if index == app.active_feed { "*" } else { " " };
        lines.push(Line::from(format!("  {marker} {}", feed.label)));
    }
    lines.push(Line::from(
        "  [ previous feed, ] or Space on this section: next feed",
    ));
    lines.push(Line::from(""));

    lines.push(section_line(MenuSection::Settings, selected));
    lines.push(Line::from(format!(
        "  images: {}",
        app.media.protocol_name()
    )));
    lines.push(Line::from("  Esc, ?, Enter, or q closes this menu"));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(rounded_block().title("Menu"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_link_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    links: Vec<crate::model::LinkRef>,
    selected: usize,
) {
    let area = centered_rect(86, 72, area);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    for (index, link) in links.iter().enumerate() {
        let style = if index == selected {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        lines.push(Line::from(vec![Span::styled(
            format!("{} [{}] {}", index + 1, link.source.label(), link.label),
            style,
        )]));
        lines.push(Line::from(vec![Span::styled(
            format!("  {}", link.uri),
            Style::default().fg(Color::DarkGray),
        )]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("Enter/u open · j/k move · Esc close"));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(rounded_block().title("Links"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_media_overlay(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &mut App,
    media: &PreviewMedia,
    selected: usize,
    total: usize,
) {
    let area = centered_rect(92, 88, area);
    frame.render_widget(Clear, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(1)])
        .split(area);

    match media {
        PreviewMedia::Image(image) => {
            let title = media_title(
                "Image",
                selected,
                total,
                image.source.label(),
                image.alt.as_deref(),
            );
            app.media
                .render_preview_image(frame, chunks[0], image, title);
        }
        PreviewMedia::Video(video) => {
            let title = media_title(
                "Video",
                selected,
                total,
                video.source.label(),
                video.alt.as_deref(),
            );
            if app.media.video_state_name(&video.playlist_url) == "missing"
                && let Some(thumb_url) = &video.thumb_url
            {
                let image = PreviewImage {
                    url: thumb_url.clone(),
                    alt: video.alt.clone(),
                    source: video.source,
                };
                app.media
                    .render_preview_image(frame, chunks[0], &image, title);
            } else {
                app.media
                    .render_preview_video(frame, chunks[0], video, title);
            }
        }
    }
    frame.render_widget(
        Paragraph::new(" h/l switch · Enter/p play video · u open · Space/Esc close ")
            .style(Style::default().fg(Color::Black).bg(Color::Gray)),
        chunks[1],
    );
}

fn render_composer_overlay(frame: &mut Frame<'_>, area: Rect, state: ComposerState) {
    let area = centered_rect(82, 54, area);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    match &state.kind {
        ComposerKind::Post => {}
        ComposerKind::Reply { parent_handle, .. } => {
            lines.push(Line::from(format!("Replying to @{parent_handle}")));
            lines.push(Line::from(""));
        }
        ComposerKind::Quote { quote_handle, .. } => {
            lines.push(Line::from(format!("Quoting @{quote_handle}")));
            lines.push(Line::from(""));
        }
    }
    if state.buffer.is_empty() {
        lines.push(Line::from(vec![Span::styled(
            "Type your post...",
            Style::default().fg(Color::DarkGray),
        )]));
    } else {
        lines.extend(state.buffer.lines().map(|line| Line::from(line.to_owned())));
    }
    lines.push(Line::from(""));
    let count = state.buffer.chars().count();
    let count_style = if count > 300 {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{count}/300"), count_style),
        Span::raw(" · Ctrl-S send · Esc cancel"),
    ]));

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(rounded_block().title(state.title()))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn section_line(section: MenuSection, selected: MenuSection) -> Line<'static> {
    let style = if section == selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };
    Line::from(vec![Span::styled(section.label(), style)])
}

fn active_feed_label(app: &App) -> &str {
    app.feeds
        .get(app.active_feed)
        .map(|feed| feed.label.as_str())
        .unwrap_or("Following")
}

fn status_line(app: &App) -> Line<'static> {
    let mut spans = vec![
        segment(
            format!(" @{} ", app.client.session().handle),
            Color::Black,
            Color::Cyan,
        ),
        Span::raw(" "),
        segment(
            format!(" {} ", active_feed_label(app)),
            Color::Black,
            Color::Yellow,
        ),
        Span::raw(" "),
        segment(
            format!(" {} ", app.nav.breadcrumb()),
            Color::White,
            Color::DarkGray,
        ),
    ];

    if app.pending_new_count() > 0 {
        spans.push(Span::raw(" "));
        spans.push(segment(
            format!(" ↑ {} new ", app.pending_new_count()),
            Color::Black,
            Color::Green,
        ));
    }

    if app.has_pending_tasks() {
        spans.push(Span::raw(" "));
        spans.push(segment(" … ".to_owned(), Color::Black, Color::Magenta));
    }

    let status = match &app.input_mode {
        InputMode::Normal => app.status.clone(),
        InputMode::Search { buffer } => format!("/{buffer}"),
    };
    if !status.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!(" {status} "),
            Style::default().fg(Color::Gray),
        ));
    }

    spans.push(Span::raw(" "));
    spans.push(segment(
        format!(" {} ", app.current_position_label()),
        Color::Black,
        Color::LightBlue,
    ));

    Line::from(spans)
}

fn segment(text: String, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        text,
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
}

#[cfg(test)]
fn normal_status_text(handle: &str, feed: &str, status: &str) -> String {
    format!(" @{handle} | {feed} | {status} ")
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.to_owned();
    }

    let mut truncated = value
        .chars()
        .take(max.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn rounded_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
}

fn media_title(
    kind: &str,
    selected: usize,
    total: usize,
    source: &str,
    alt: Option<&str>,
) -> String {
    let alt = alt
        .map(|alt| format!(" · {}", truncate(alt, 40)))
        .unwrap_or_default();
    format!("{kind} {}/{} · {source}{alt}", selected + 1, total)
}

fn render_item_lines(item: &FeedItem, selected: bool, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let marker = if selected { ">" } else { " " };
    let indent = "  ".repeat(item.depth.min(6));
    let time = compact_time(item.indexed_at.as_deref());
    let header_style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };

    if let Some(reason) = &item.reason {
        lines.push(Line::from(vec![Span::styled(
            format!("  {indent}{}", reason_text(reason)),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    if let Some(reply) = &item.reply {
        lines.extend(reply_preview_lines(reply, width, &format!("  {indent}")));
    }

    lines.push(Line::from(vec![Span::styled(
        format!(
            "{marker} {indent}{} @{} {}",
            item.author_name, item.author_handle, time
        ),
        header_style,
    )]));

    let body_prefix = format!("  {indent}");
    for line in wrap_text(&item.text, width.saturating_sub(body_prefix.len()).max(10)) {
        lines.push(Line::from(format!("{body_prefix}{line}")));
    }

    render_media_summary(
        &mut lines,
        &body_prefix,
        &item.images,
        &item.videos,
        item.external.as_ref(),
    );

    if let Some(quote) = &item.quote {
        render_quote_lines(&mut lines, quote, width, &body_prefix);
    }

    if let Some(status) = &item.embed_status {
        lines.push(Line::from(format!("{body_prefix}{status}")));
    }

    lines.push(Line::from(format!(
        "{body_prefix}{}",
        engagement_summary(item)
    )));
    lines.push(Line::from(""));
    lines
}

fn reason_text(reason: &FeedReason) -> String {
    match reason {
        FeedReason::Repost {
            by_handle,
            indexed_at,
            ..
        } => {
            let time = compact_time(indexed_at.as_deref());
            if time.is_empty() {
                format!("⟳ @{by_handle} reposted")
            } else {
                format!("⟳ @{by_handle} reposted {time}")
            }
        }
        FeedReason::Pin => "⚑ pinned".into(),
    }
}

fn reply_preview_lines(reply: &ReplyContext, width: usize, prefix: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let grandparent = reply
        .grandparent_author_handle
        .as_ref()
        .map(|handle| format!(" via @{handle}"))
        .unwrap_or_default();
    let label = match reply.parent_status {
        Some(ReplyParentStatus::Blocked) => "↩ replying to blocked post".to_owned(),
        Some(ReplyParentStatus::NotFound) => "↩ replying to missing post".to_owned(),
        None => format!(
            "↩ replying to @{}{}",
            reply.parent_author_handle, grandparent
        ),
    };
    lines.push(Line::from(vec![Span::styled(
        format!("{prefix}{label}"),
        Style::default().fg(Color::DarkGray),
    )]));

    let preview_prefix = format!("{prefix}│ ");
    for line in wrap_text(
        &reply.parent_text,
        width.saturating_sub(preview_prefix.len()).max(10),
    )
    .into_iter()
    .take(2)
    {
        lines.push(Line::from(vec![Span::styled(
            format!("{preview_prefix}{line}"),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    lines
}

pub(crate) fn engagement_summary(item: &FeedItem) -> String {
    format!(
        "↩ {}  ⟳ {}  ♥ {}  ❞ {}",
        item.reply_count, item.repost_count, item.like_count, item.quote_count
    )
}

fn render_quote_lines(
    lines: &mut Vec<Line<'static>>,
    quote: &QuotePost,
    width: usize,
    body_prefix: &str,
) {
    let quote_prefix = format!("{body_prefix}| ");
    lines.push(Line::from(vec![Span::styled(
        format!(
            "{body_prefix}+-- quote {} @{} {}",
            quote.author_name,
            quote.author_handle,
            compact_time(quote.indexed_at.as_deref())
        ),
        Style::default().fg(Color::Yellow),
    )]));
    for line in wrap_text(
        &quote.text,
        width.saturating_sub(quote_prefix.len()).max(10),
    ) {
        lines.push(Line::from(format!("{quote_prefix}{line}")));
    }
    render_media_summary(
        lines,
        &quote_prefix,
        &quote.images,
        &quote.videos,
        quote.external.as_ref(),
    );
    if let Some(nested) = &quote.nested_quote {
        lines.push(Line::from(format!("{quote_prefix}nested quote: {nested}")));
    }
}

fn render_media_summary(
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    images: &[ImageRef],
    videos: &[crate::model::VideoRef],
    external: Option<&ExternalRef>,
) {
    if !images.is_empty() {
        let label = if images.len() == 1 { "image" } else { "images" };
        let alt = images
            .first()
            .and_then(|image| image.alt.as_ref())
            .map(|alt| format!(": {alt}"))
            .unwrap_or_default();
        lines.push(Line::from(format!(
            "{prefix}[{} {label}{alt}]",
            images.len()
        )));
    }
    if !videos.is_empty() {
        let label = if videos.len() == 1 { "video" } else { "videos" };
        let alt = videos
            .first()
            .and_then(|video| video.alt.as_ref())
            .map(|alt| format!(": {alt}"))
            .unwrap_or_default();
        lines.push(Line::from(format!(
            "{prefix}[{} {label}{alt}]",
            videos.len()
        )));
    }
    if let Some(external) = external {
        let description = external
            .description
            .as_ref()
            .map(|description| format!(" - {description}"))
            .unwrap_or_default();
        lines.push(Line::from(format!(
            "{prefix}[link] {}{}",
            external.title, description
        )));
    }
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }

    text.lines()
        .flat_map(|line| {
            textwrap::wrap(line, width.max(10))
                .into_iter()
                .map(|line| line.into_owned())
                .collect::<Vec<_>>()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::navigation::ViewKind;

    fn item() -> FeedItem {
        item_with_text("hello")
    }

    fn item_with_text(text: &str) -> FeedItem {
        FeedItem {
            uri: "at://did:plc:alice/app.bsky.feed.post/1".into(),
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
            reply_count: 2,
            repost_count: 3,
            like_count: 5,
            quote_count: 7,
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
    fn renders_unicode_engagement_summary() {
        assert_eq!(engagement_summary(&item()), "↩ 2  ⟳ 3  ♥ 5  ❞ 7");
    }

    #[test]
    fn renders_repost_reason_text() {
        let reason = FeedReason::Repost {
            by_name: "Alice".into(),
            by_handle: "alice.test".into(),
            indexed_at: None,
        };
        assert_eq!(reason_text(&reason), "⟳ @alice.test reposted");
    }

    #[test]
    fn status_line_is_compact_and_has_no_footer_controls() {
        let text = normal_status_text("alice.test", "Following", "Loaded");

        assert_eq!(text, " @alice.test | Following | Loaded ");
        assert!(!text.contains("j/k"));
        assert!(!text.contains("replies"));
        assert!(!text.contains("img:"));
    }

    #[test]
    fn scrolls_when_selected_variable_height_item_is_below_viewport() {
        let tall_text = "line 1\nline 2\nline 3";
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![
                item_with_text(tall_text),
                item_with_text(tall_text),
                item_with_text(tall_text),
                item_with_text(tall_text),
            ],
        );
        view.selected = 3;
        view.scroll = 0;

        ensure_selected_rendered(&mut view, 80, 12);

        assert_eq!(view.scroll, 2);
    }

    #[test]
    fn scrolls_selected_oversized_item_to_top() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![
                item_with_text("short"),
                item_with_text(&"line\n".repeat(20)),
            ],
        );
        view.selected = 1;
        view.scroll = 0;

        ensure_selected_rendered(&mut view, 80, 8);

        assert_eq!(view.scroll, 1);
    }

    #[test]
    fn scrolling_up_restores_selected_as_top_when_needed() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![
                item_with_text("one"),
                item_with_text("two"),
                item_with_text("three"),
                item_with_text("four"),
            ],
        );
        view.selected = 1;
        view.scroll = 3;

        ensure_selected_rendered(&mut view, 80, 12);

        assert_eq!(view.scroll, 1);
    }

    #[test]
    fn renders_partial_next_item_instead_of_blank_space() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![
                item_with_text("short"),
                item_with_text(&"long line\n".repeat(20)),
            ],
        );

        let lines = visible_feed_lines(&mut view, 80, 6);

        assert_eq!(lines.len(), 6);
    }

    #[test]
    fn reuses_layout_cache_for_same_width() {
        let mut view = ViewState::new(
            "Timeline",
            ViewKind::Timeline,
            vec![item_with_text("one"), item_with_text("two")],
        );

        let _ = visible_feed_lines(&mut view, 80, 10);
        let first_builds = view.layout_cache.builds;
        let _ = visible_feed_lines(&mut view, 80, 10);

        assert_eq!(first_builds, 1);
        assert_eq!(view.layout_cache.builds, first_builds);

        let _ = visible_feed_lines(&mut view, 40, 10);
        assert_eq!(view.layout_cache.builds, first_builds + 1);
    }
}
