use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::{App, ComposerKind, ComposerState, InputMode, KeyHelpGroup, Overlay, key_help_groups},
    media::{PreviewImage, PreviewMedia},
    model::{
        ExternalRef, FeedItem, FeedReason, ImageRef, NotificationItem, NotificationReason,
        ProfileSummary, QuotePost, ReplyParentStatus, ViewItem, compact_time,
    },
    navigation::{CachedItemLines, ViewKind, ViewState},
};

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(5), Constraint::Length(status_height())])
        .split(area);

    render_body(frame, chunks[0], app);
    render_status(frame, chunks[1], app);
    render_overlay(frame, area, app);
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    render_feed(frame, area, app);
}

fn render_feed(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let active_did = app.client.session().did.clone();
    let view = app.nav.current_mut();
    let title = if view.loading || view.page_request.is_some() {
        format!("{} [loading]", view.title)
    } else {
        view.title.clone()
    };
    let block = rounded_block().title(title);
    let inner_width = area.width.saturating_sub(4).max(12) as usize;
    let available_lines = area.height.saturating_sub(2) as usize;
    let profile_lines = view
        .profile
        .as_ref()
        .map(|profile| render_profile_header_lines(profile, inner_width, &active_did))
        .unwrap_or_default();
    // Reserve the error row before laying out the list; appended after a
    // full page of posts it was clipped and never seen.
    let error_line = view.error.as_ref().map(|error| {
        Line::from(vec![Span::styled(
            truncate(&format!("⚠ {}", single_line(error)), inner_width + 2),
            Style::default().fg(Color::Red),
        )])
    });
    let list_available = available_lines
        .saturating_sub(profile_lines.len())
        .saturating_sub(usize::from(error_line.is_some()));
    let mut lines = profile_lines;
    lines.extend(visible_feed_lines(view, inner_width, list_available));
    lines.extend(error_line);

    frame.render_widget(Paragraph::new(Text::from(lines)).block(block), area);
}

pub(crate) fn current_minute() -> i64 {
    chrono::Utc::now().timestamp() / 60
}

fn visible_feed_lines(
    view: &mut ViewState,
    width: usize,
    available_lines: usize,
) -> Vec<Line<'static>> {
    visible_feed_lines_at(view, width, available_lines, current_minute())
}

fn visible_feed_lines_at(
    view: &mut ViewState,
    width: usize,
    available_lines: usize,
    minute: i64,
) -> Vec<Line<'static>> {
    ensure_layout_cache(view, width, minute);
    ensure_selected_rendered(view, width, available_lines);

    if view.items.is_empty() {
        return vec![Line::from(empty_view_text(&view.kind))];
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

fn ensure_layout_cache(view: &mut ViewState, width: usize, minute: i64) {
    let needs_rebuild = view.layout_cache.width != Some(width)
        || view.layout_cache.minute != Some(minute)
        || view.layout_cache.items.len() != view.items.len();
    if !needs_rebuild {
        return;
    }

    view.layout_cache.width = Some(width);
    view.layout_cache.minute = Some(minute);
    let contexts = row_contexts(view);
    view.layout_cache.items = view
        .items
        .iter()
        .zip(&contexts)
        .map(|(item, context)| CachedItemLines {
            selected: render_view_item_lines(item, true, width, context),
            unselected: render_view_item_lines(item, false, width, context),
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
        let contexts = row_contexts(view);
        return (start..=end)
            .map(|index| {
                render_view_item_lines(&view.items[index], false, width, &contexts[index]).len()
            })
            .sum();
    }

    view.layout_cache.items[start..=end]
        .iter()
        .map(|item| item.unselected.len())
        .sum()
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App) {
    // No filled bar: the list's bottom border already separates it, and a
    // row of saturated blocks competes with the content above.
    render_status_content(frame, area, app);
}

fn render_status_content(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let right = status_right_line(app);
    let right_width = line_width(&right).min(area.width as usize) as u16;
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(right_width)])
        .split(area);
    let left = status_left_line(app, chunks[0].width as usize);

    frame.render_widget(Paragraph::new(left), chunks[0]);
    frame.render_widget(Paragraph::new(right), chunks[1]);
}

pub(crate) fn status_height() -> u16 {
    1
}

fn render_overlay(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let Some(overlay) = app.overlay.clone() else {
        return;
    };

    match overlay {
        Overlay::Menu(state) => render_menu_overlay(frame, area, app, state.selected),
        Overlay::Media(state) => {
            if let Some(media) = state.selected_media().cloned() {
                render_media_overlay(frame, area, app, &media, state.selected, state.media.len());
            }
        }
        Overlay::Links(state) => render_link_overlay(frame, area, state.links, state.selected),
        Overlay::Composer(state) => render_composer_overlay(frame, area, state),
        Overlay::ConfirmDelete(_) => render_confirm_delete_overlay(frame, area),
        Overlay::Switcher(state) => render_switcher_overlay(frame, area, app, state.selected),
    }
}

fn render_confirm_delete_overlay(frame: &mut Frame<'_>, area: Rect) {
    let area = centered_rect(60, 20, area);
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from("This permanently deletes the post from Bluesky."),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "y",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" delete · any other key cancels"),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(rounded_block().title("Delete post?"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// The menu (Esc on the timeline, or `?`): feeds and accounts to switch
/// between, the key reference, and settings, in one place.
fn render_menu_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let (feeds, accounts) = switch_lists(app, selected);
    let list_rows = feeds.len().max(accounts.len()) as u16;
    let rect = centered_fixed(80, 2 + list_rows + 1 + KEY_CARD_ROWS + 1 + 1, area);
    frame.render_widget(Clear, rect);
    let block = rounded_block()
        .title(" Menu ")
        .border_style(Style::default().fg(MUTED));
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(list_rows),
            Constraint::Length(1),
            Constraint::Length(KEY_CARD_ROWS),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let lists = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 2); 2])
        .split(chunks[0]);
    let margin = ratatui::layout::Margin::new(1, 0);
    frame.render_widget(Paragraph::new(Text::from(feeds)), lists[0].inner(margin));
    frame.render_widget(Paragraph::new(Text::from(accounts)), lists[1].inner(margin));
    render_key_card(frame, chunks[2]);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" images: ", Style::default().fg(MUTED)),
            Span::raw(app.media.protocol_name()),
            Span::styled("   ", Style::default()),
            Span::styled("●", Style::default().fg(Color::Green)),
            Span::styled(
                " current · j/k move · Enter switch · Esc close",
                Style::default().fg(MUTED),
            ),
        ])),
        chunks[4],
    );
}

const KEY_CARD_ROWS: u16 = 14;

/// The grouped key reference (which-key style), two rows of three groups.
fn render_key_card(frame: &mut Frame<'_>, area: Rect) {
    let groups = key_help_groups();
    let key_style = Style::default().fg(ACCENT).add_modifier(Modifier::BOLD);
    let head_style = Style::default().fg(MUTED).add_modifier(Modifier::BOLD);
    let column = |group: &KeyHelpGroup| {
        let mut lines = vec![Line::from(Span::styled(group.0.to_owned(), head_style))];
        for (key, label) in group.1 {
            lines.push(Line::from(vec![
                Span::styled(format!("{key:<10}"), key_style),
                Span::raw(label.to_string()),
            ]));
        }
        lines
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(KEY_CARD_ROWS / 2); 2])
        .split(area);
    for (row, chunk) in groups.chunks(3).enumerate() {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 3); 3])
            .split(rows[row]);
        for (index, group) in chunk.iter().enumerate() {
            let area = cols[index].inner(ratatui::layout::Margin::new(1, 0));
            frame.render_widget(Paragraph::new(Text::from(column(group))), area);
        }
    }
}

/// Feed and account rows shared by the menu and the Tab switcher. Feeds
/// come first, then accounts; `selected` indexes across both.
fn switch_lists(app: &App, selected: usize) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
    let head = |label: &str| {
        Line::from(Span::styled(
            label.to_owned(),
            Style::default().fg(MUTED).add_modifier(Modifier::BOLD),
        ))
    };
    let row = |index: usize, label: String, active: bool| {
        let (marker, style) = if index == selected {
            (
                Span::styled("▌ ", Style::default().fg(ACCENT)),
                Style::default().add_modifier(Modifier::BOLD),
            )
        } else {
            (Span::raw("  "), Style::default())
        };
        let mut spans = vec![marker, Span::styled(label, style)];
        if active {
            spans.push(Span::styled(" ●", Style::default().fg(Color::Green)));
        }
        Line::from(spans)
    };
    let mut feeds = vec![head("FEEDS")];
    feeds.extend(
        app.feeds
            .iter()
            .enumerate()
            .map(|(index, feed)| row(index, feed.label.clone(), index == app.active_feed)),
    );
    let current_did = app.client.session().did;
    let mut accounts = vec![head("ACCOUNTS")];
    accounts.extend(app.accounts.iter().enumerate().map(|(index, account)| {
        row(
            app.feeds.len() + index,
            format!("@{}", account.session.handle),
            account.session.did == current_did,
        )
    }));
    (feeds, accounts)
}

fn render_switcher_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, selected: usize) {
    let (feeds, accounts) = switch_lists(app, selected);
    let mut lines = feeds;
    lines.push(Line::from(""));
    lines.extend(accounts);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "● current · j/k move · Enter switch · Esc close",
        Style::default().fg(MUTED),
    )));
    let rect = centered_fixed(52, lines.len() as u16 + 2, area);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).block(
            rounded_block()
                .title(" Switch ")
                .border_style(Style::default().fg(MUTED)),
        ),
        rect,
    );
}

fn centered_fixed(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
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
                    thumb_url: None,
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
    let muted = Style::default().fg(MUTED);
    let title = match &state.kind {
        ComposerKind::Post => " New post ".to_owned(),
        ComposerKind::Reply { parent_handle, .. } => format!(" Reply to @{parent_handle} "),
        ComposerKind::Quote { quote_handle, .. } => format!(" Quote @{quote_handle} "),
    };
    let count = crate::app::post_grapheme_count(&state.buffer);
    let count_style = match count {
        0..=279 => muted,
        280..=300 => Style::default().fg(ACCENT),
        _ => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
    };
    let mut lines = Vec::new();
    if state.buffer.is_empty() {
        lines.push(Line::from(vec![
            Span::styled("▏", Style::default().fg(ACCENT)),
            Span::styled("What's up?", muted),
        ]));
    } else {
        let mut body = state
            .buffer
            .split('\n')
            .map(|line| Line::from(rich_text_spans(line)))
            .collect::<Vec<_>>();
        if let Some(last) = body.last_mut() {
            last.spans
                .push(Span::styled("▏", Style::default().fg(ACCENT)));
        }
        lines.extend(body);
    }
    let block = rounded_block()
        .title(title)
        .title_bottom(Line::from(vec![Span::styled(
            " ^S send · Esc keep draft · ^E $EDITOR ",
            muted,
        )]))
        .title_bottom(
            Line::from(Span::styled(format!(" {count}/300 "), count_style)).right_aligned(),
        )
        .border_style(Style::default().fg(MUTED));
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn active_feed_label(app: &App) -> &str {
    app.feeds
        .get(app.active_feed)
        .map(|feed| feed.label.as_str())
        .unwrap_or("Following")
}

pub(crate) fn status_left_line(app: &App, max_width: usize) -> Line<'static> {
    if max_width == 0 {
        return Line::from("");
    }

    let with_status = status_left_line_inner(app, true);
    if line_width(&with_status) <= max_width {
        return with_status;
    }

    status_left_line_inner(app, false)
}

fn short_handle(handle: &str) -> &str {
    handle.strip_suffix(".bsky.social").unwrap_or(handle)
}

fn status_left_line_inner(app: &App, include_transient_status: bool) -> Line<'static> {
    let bold = |color: Color| Style::default().fg(color).add_modifier(Modifier::BOLD);
    let mut spans = vec![
        Span::styled(
            format!(" @{}", short_handle(&app.client.session().handle)),
            bold(Color::Cyan),
        ),
        Span::styled("  ", Style::default()),
    ];
    let root = active_feed_label(app).to_owned();
    if app.nav.depth() > 1 {
        spans.push(Span::styled(root, Style::default().fg(MUTED)));
        if app.nav.depth() > 2 {
            spans.push(Span::styled(" › …", Style::default().fg(MUTED)));
        }
        spans.push(Span::styled(" › ", Style::default().fg(MUTED)));
        spans.push(Span::styled(
            app.nav.current().title.clone(),
            bold(Color::White),
        ));
    } else {
        spans.push(Span::styled(root, bold(Color::White)));
    }

    if app.unread_notifications > 0 {
        spans.push(Span::styled(
            format!("  ● {}", app.unread_notifications),
            bold(Color::LightRed),
        ));
    }
    if app.pending_new_count() > 0 {
        spans.push(Span::styled(
            format!("  ↑ {} new", app.pending_new_count()),
            bold(Color::Green),
        ));
    }
    if app.is_offline() {
        spans.push(Span::styled(
            "  ⚠ offline",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.has_pending_tasks() {
        spans.push(Span::styled(
            format!("  {}", app.spinner_char()),
            Style::default().fg(Color::Magenta),
        ));
    }

    let status = if include_transient_status {
        status_text(app)
    } else {
        String::new()
    };
    if !status.is_empty() {
        let style = if app.status_is_error {
            Style::default().fg(Color::LightRed)
        } else {
            Style::default().fg(MUTED)
        };
        spans.push(Span::styled(format!("  {status}"), style));
    }

    Line::from(spans)
}

fn status_text(app: &App) -> String {
    match (&app.input_mode, &app.overlay) {
        (InputMode::Search { buffer }, _) => format!("/{buffer}"),
        (InputMode::Normal, Some(Overlay::Composer(state))) => state.title().to_owned(),
        (InputMode::Normal, _) => app
            .visible_status()
            .or_else(|| app.pending_task_label())
            .unwrap_or_default()
            .to_owned(),
    }
}

pub(crate) fn status_right_line(app: &App) -> Line<'static> {
    Line::from(vec![
        Span::styled(app.current_position_label(), Style::default().fg(MUTED)),
        Span::styled("  ? menu ", Style::default().fg(MUTED)),
    ])
}

#[cfg(test)]
pub(crate) fn current_location_label(app: &App) -> String {
    if app.nav.depth() > 1 {
        app.nav.current().title.clone()
    } else {
        active_feed_label(app).to_owned()
    }
}

pub(crate) fn line_width(line: &Line<'_>) -> usize {
    line.width()
}

#[cfg(test)]
fn normal_status_text(handle: &str, feed: &str, status: &str) -> String {
    format!(" @{handle}  {feed}  {status} ")
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

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
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

fn empty_view_text(kind: &ViewKind) -> &'static str {
    match kind {
        ViewKind::Notifications => "No notifications.",
        ViewKind::Profile { .. } => "No posts for this profile.",
        _ => "No posts in this view.",
    }
}

// ---- Row rendering -------------------------------------------------------
//
// Every row is drawn as `gutter + content`. The two-column gutter carries the
// selection bar down the full height of the selected item, so the extent of
// a multi-line post is obvious; colour is kept for information (who, what you
// did, what is a link) rather than decoration.

const ACCENT: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;
const LINK: Color = Color::LightBlue;
const MENTION: Color = Color::LightCyan;
const TAG: Color = Color::LightMagenta;
const FRAME: Color = Color::DarkGray;
const MAX_THREAD_DEPTH: usize = 6;
/// Stable per-account name colours (irssi/weechat nick colours). Yellow is
/// reserved for selection and red for likes/errors.
const NICK_COLORS: [Color; 7] = [
    Color::Cyan,
    Color::Green,
    Color::Magenta,
    Color::LightBlue,
    Color::LightCyan,
    Color::LightMagenta,
    Color::LightGreen,
];

pub(crate) fn nick_color(key: &str) -> Color {
    // FNV-1a: stable across runs and Rust versions, unlike DefaultHasher.
    let mut hash: u32 = 0x811c_9dc5;
    for byte in key.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    NICK_COLORS[hash as usize % NICK_COLORS.len()]
}

fn display_width(text: &str) -> usize {
    Span::raw(text.to_owned()).width()
}

fn truncate_width(text: &str, max: usize) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    if display_width(text) <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        let width = display_width(grapheme);
        if used + width + 1 > max {
            break;
        }
        out.push_str(grapheme);
        used += width;
    }
    out.push('…');
    out
}

pub(crate) fn compact_count(count: u64) -> String {
    match count {
        0..=999 => count.to_string(),
        1_000..=9_999 => format!("{:.1}K", count as f64 / 1_000.0).replace(".0K", "K"),
        10_000..=999_999 => format!("{}K", count / 1_000),
        _ => format!("{:.1}M", count as f64 / 1_000_000.0).replace(".0M", "M"),
    }
}

/// Per-row context the row itself cannot know: thread tree guides and
/// whether this is the post the thread was opened on.
#[derive(Debug, Clone, Default)]
struct RowContext {
    header_guide: String,
    body_guide: String,
    anchor: bool,
}

fn row_contexts(view: &ViewState) -> Vec<RowContext> {
    let anchor_uri = match &view.kind {
        ViewKind::Thread { root_uri } => Some(root_uri.as_str()),
        ViewKind::Quote { uri } => Some(uri.as_str()),
        _ => None,
    };
    let Some(anchor_uri) = anchor_uri else {
        return vec![RowContext::default(); view.items.len()];
    };
    let depths = view
        .items
        .iter()
        .map(|item| item.as_post().map_or(0, |post| post.depth))
        .collect::<Vec<_>>();
    thread_guides(&depths)
        .into_iter()
        .zip(&view.items)
        .map(|((header_guide, body_guide), item)| RowContext {
            header_guide,
            body_guide,
            anchor: item.uri() == anchor_uri,
        })
        .collect()
}

/// Tree connectors for a depth-first flattened thread (slrn/mutt style).
/// Each level is three columns; a body line also reserves two columns for
/// the line that leads down to this post's own replies.
pub(crate) fn thread_guides(depths: &[usize]) -> Vec<(String, String)> {
    let depths = depths
        .iter()
        .map(|depth| (*depth).min(MAX_THREAD_DEPTH))
        .collect::<Vec<_>>();
    let later_at = |index: usize, level: usize| {
        for depth in &depths[index + 1..] {
            if *depth < level {
                return false;
            }
            if *depth == level {
                return true;
            }
        }
        false
    };
    (0..depths.len())
        .map(|index| {
            let depth = depths[index];
            let mut header = String::new();
            let mut body = String::new();
            for level in 1..depth {
                let column = if later_at(index, level) {
                    "│  "
                } else {
                    "   "
                };
                header.push_str(column);
                body.push_str(column);
            }
            if depth >= 1 {
                let last = !later_at(index, depth);
                header.push_str(if last { "└─ " } else { "├─ " });
                body.push_str(if last { "   " } else { "│  " });
            }
            let has_replies = depths.get(index + 1).is_some_and(|next| *next > depth);
            let chain_continues = depth == 0 && later_at(index, 0);
            body.push_str(if has_replies || chain_continues {
                "│ "
            } else {
                "  "
            });
            (header, body)
        })
        .collect()
}

fn with_gutter(lines: Vec<Line<'static>>, selected: bool) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let bar = if selected {
                Span::styled("▌ ", Style::default().fg(ACCENT))
            } else {
                Span::raw("  ")
            };
            let mut spans = vec![bar];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

fn guide_span(guide: &str) -> Span<'static> {
    Span::styled(guide.to_owned(), Style::default().fg(FRAME))
}

/// `Name @handle ··· 5m` with the time right-aligned. A display name that is
/// just the handle is shown once.
fn author_header(
    lead: Vec<Span<'static>>,
    name: &str,
    handle: &str,
    color_key: &str,
    time: &str,
    width: usize,
) -> Line<'static> {
    let lead_width = lead.iter().map(Span::width).sum::<usize>();
    let time_width = if time.is_empty() {
        0
    } else {
        display_width(time) + 1
    };
    let room = width.saturating_sub(lead_width + time_width);
    let name_style = Style::default()
        .fg(nick_color(color_key))
        .add_modifier(Modifier::BOLD);
    let mut spans = lead;
    let handle_text = format!("@{handle}");
    let mut used = 0;
    if name.trim().is_empty() || name == handle {
        let shown = truncate_width(&handle_text, room);
        used += display_width(&shown);
        spans.push(Span::styled(shown, name_style));
    } else {
        let shown_name = truncate_width(name, room.saturating_sub(2).max(8));
        used += display_width(&shown_name);
        spans.push(Span::styled(shown_name, name_style));
        let handle_room = room.saturating_sub(used + 1);
        if handle_room >= 4 {
            let shown = format!(" {}", truncate_width(&handle_text, handle_room));
            used += display_width(&shown);
            spans.push(Span::styled(shown, Style::default().fg(MUTED)));
        }
    }
    if !time.is_empty() {
        spans.push(Span::raw(" ".repeat(room.saturating_sub(used) + 1)));
        spans.push(Span::styled(time.to_owned(), Style::default().fg(MUTED)));
    }
    Line::from(spans)
}

/// Colours @mentions, #tags and links inside an already-wrapped line.
fn rich_text_spans(line: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for token in line.split_inclusive(' ') {
        let word = token
            .trim_end()
            .trim_end_matches(['.', ',', ':', ';', '!', '?', ')', '"', '\'', '”', '’']);
        let style = if word.len() > 3 && word.starts_with('@') && word.contains('.') {
            Some(Style::default().fg(MENTION))
        } else if word.len() > 1 && word.starts_with('#') && !word[1..].starts_with('#') {
            Some(Style::default().fg(TAG))
        } else if word.starts_with("http://")
            || word.starts_with("https://")
            || word.starts_with("www.")
            || (word.contains('.') && word.contains('/') && !word.contains('@'))
        {
            Some(Style::default().fg(LINK).add_modifier(Modifier::UNDERLINED))
        } else {
            None
        };
        match style {
            Some(style) => {
                let tail = &token[word.len()..];
                spans.push(Span::styled(word.to_owned(), style));
                if !tail.is_empty() {
                    spans.push(Span::raw(tail.to_owned()));
                }
            }
            None => match spans.last_mut() {
                Some(last) if last.style == Style::default() => {
                    last.content = format!("{}{token}", last.content).into();
                }
                _ => spans.push(Span::raw(token.to_owned())),
            },
        }
    }
    spans
}

fn body_lines(text: &str, prefix: &[Span<'static>], width: usize) -> Vec<Line<'static>> {
    let prefix_width = prefix.iter().map(Span::width).sum::<usize>();
    if text.trim().is_empty() {
        return Vec::new();
    }
    wrap_text(text, width.saturating_sub(prefix_width).max(10))
        .into_iter()
        .map(|line| {
            let mut spans = prefix.to_vec();
            spans.extend(rich_text_spans(&line));
            Line::from(spans)
        })
        .collect()
}

fn render_profile_header_lines(
    profile: &ProfileSummary,
    width: usize,
    active_did: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let pill = if !profile.did.is_empty() && profile.did != active_did {
        if profile.viewer_following.is_some() {
            Some(Span::styled(
                "✓ following",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ))
        } else {
            Some(Span::styled(
                "not following · F to follow",
                Style::default().fg(MUTED),
            ))
        }
    } else {
        None
    };
    let pill_width = pill.as_ref().map_or(0, |pill| pill.width() + 1);
    let mut header = author_header(
        vec![Span::raw("  ")],
        &profile.display_name,
        &profile.handle,
        &profile.did,
        "",
        width.saturating_sub(pill_width),
    );
    if let Some(pill) = pill {
        let used = header.width();
        header.spans.push(Span::raw(
            " ".repeat((width + 2).saturating_sub(used + pill.width())),
        ));
        header.spans.push(pill);
    }
    lines.push(header);
    if let Some(description) = &profile.description {
        for line in wrap_text(description, width.saturating_sub(2))
            .into_iter()
            .take(3)
        {
            let mut spans = vec![Span::raw("  ")];
            spans.extend(rich_text_spans(&line));
            lines.push(Line::from(spans));
        }
    }
    let count = |value: u64, label: &str| {
        vec![
            Span::styled(
                compact_count(value),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!(" {label}"), Style::default().fg(MUTED)),
        ]
    };
    let mut stats = vec![Span::raw("  ")];
    stats.extend(count(profile.followers_count, "followers"));
    stats.push(Span::styled(" · ", Style::default().fg(MUTED)));
    stats.extend(count(profile.follows_count, "following"));
    stats.push(Span::styled(" · ", Style::default().fg(MUTED)));
    stats.extend(count(profile.posts_count, "posts"));
    lines.push(Line::from(stats));
    lines.push(Line::from(Span::styled(
        format!("  {}", "─".repeat(width.saturating_sub(2))),
        Style::default().fg(FRAME),
    )));
    lines
}

fn render_view_item_lines(
    item: &ViewItem,
    selected: bool,
    width: usize,
    context: &RowContext,
) -> Vec<Line<'static>> {
    let (lines, separator) = match item {
        ViewItem::Post(item) => render_item_lines(item, width, context),
        ViewItem::Notification(item) => (render_notification_lines(item, width), Line::from("")),
    };
    let mut lines = with_gutter(lines, selected);
    lines.push(separator);
    lines
}

fn notification_glyph(reason: &NotificationReason) -> (&'static str, Color) {
    match reason {
        NotificationReason::Like | NotificationReason::LikeViaRepost => ("♥", Color::Red),
        NotificationReason::Repost | NotificationReason::RepostViaRepost => ("⟳", Color::Green),
        NotificationReason::Follow
        | NotificationReason::StarterpackJoined
        | NotificationReason::ContactMatch => ("+", Color::Cyan),
        NotificationReason::Reply => ("↩", Color::LightBlue),
        NotificationReason::Mention => ("@", Color::LightCyan),
        NotificationReason::Quote => ("❞", Color::LightMagenta),
        NotificationReason::Verified | NotificationReason::Unverified => ("✓", Color::LightBlue),
        NotificationReason::SubscribedPost | NotificationReason::Unknown(_) => ("•", MUTED),
    }
}

fn notification_actors(item: &NotificationItem) -> String {
    let first = if item.author_name.trim().is_empty() || item.author_name == item.author_handle {
        format!("@{}", item.author_handle)
    } else {
        item.author_name.clone()
    };
    match item.others.as_slice() {
        [] => first,
        [other] => format!("{first} and {other}"),
        [other, last] => format!("{first}, {other} and {last}"),
        [other, rest @ ..] => format!("{first}, {other} and {} others", rest.len()),
    }
}

fn render_notification_lines(item: &NotificationItem, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let (glyph, glyph_color) = notification_glyph(&item.reason);
    let unread = if item.is_read {
        Span::raw("  ")
    } else {
        Span::styled("● ", Style::default().fg(Color::LightRed))
    };
    let lead = vec![
        unread,
        Span::styled(
            format!("{glyph} "),
            Style::default()
                .fg(glyph_color)
                .add_modifier(Modifier::BOLD),
        ),
    ];
    let actors = notification_actors(item);
    let verb = format!(" {}", item.reason.label());
    let time = compact_time(Some(&item.indexed_at));
    let room = width.saturating_sub(4 + display_width(&time) + 1);
    let actors_shown = truncate_width(&actors, room.saturating_sub(display_width(&verb)).max(12));
    let verb_shown = truncate_width(&verb, room.saturating_sub(display_width(&actors_shown)));
    let used = 4 + display_width(&actors_shown) + display_width(&verb_shown);
    let mut header = lead;
    let name_style = Style::default()
        .fg(nick_color(
            item.author_did.as_deref().unwrap_or(&item.author_handle),
        ))
        .add_modifier(if item.is_read {
            Modifier::empty()
        } else {
            Modifier::BOLD
        });
    header.push(Span::styled(actors_shown, name_style));
    header.push(Span::raw(verb_shown));
    header.push(Span::raw(
        " ".repeat(width.saturating_sub(used + display_width(&time)).max(1)),
    ));
    header.push(Span::styled(time, Style::default().fg(MUTED)));
    lines.push(Line::from(header));

    let indent = [Span::raw("    ")];
    if !item.text.trim().is_empty() {
        lines.extend(body_lines(&item.text, &indent, width).into_iter().take(3));
    } else if let Some(subject) = &item.subject_text {
        let quoted = truncate_width(&single_line(subject), width.saturating_sub(6));
        lines.push(Line::from(vec![
            Span::raw("    "),
            Span::styled(format!("“{quoted}”"), Style::default().fg(MUTED)),
        ]));
    }
    lines
}

fn render_item_lines(
    item: &FeedItem,
    width: usize,
    context: &RowContext,
) -> (Vec<Line<'static>>, Line<'static>) {
    let mut lines = Vec::new();
    let header_guide = guide_span(&context.header_guide);
    let body_guide = guide_span(&context.body_guide);
    let body_prefix = vec![body_guide.clone()];
    let time = compact_time(item.indexed_at.as_deref());
    let muted = Style::default().fg(MUTED);

    if let Some(reason) = &item.reason {
        let line = match reason {
            FeedReason::Repost {
                by_name, by_handle, ..
            } => {
                let by = if by_name.trim().is_empty() || by_name == by_handle {
                    format!("@{by_handle}")
                } else {
                    by_name.clone()
                };
                vec![
                    Span::styled("⟳ ", Style::default().fg(Color::Green)),
                    Span::styled(format!("reposted by {by}"), muted),
                ]
            }
            FeedReason::Pin => vec![Span::styled("⚑ pinned", muted)],
        };
        let mut spans = vec![header_guide.clone()];
        spans.extend(line);
        lines.push(Line::from(spans));
    }

    if let Some(reply) = &item.reply {
        let label = match reply.parent_status {
            Some(ReplyParentStatus::Blocked) => "↩ replying to a blocked post".to_owned(),
            Some(ReplyParentStatus::NotFound) => "↩ replying to a deleted post".to_owned(),
            None => format!("↩ replying to @{}", reply.parent_author_handle),
        };
        lines.push(Line::from(vec![
            header_guide.clone(),
            Span::styled(label, muted),
        ]));
        if reply.parent_status.is_none() && !reply.parent_text.trim().is_empty() {
            let preview = truncate_width(&single_line(&reply.parent_text), width.saturating_sub(4));
            lines.push(Line::from(vec![
                header_guide.clone(),
                Span::styled("│ ", Style::default().fg(FRAME)),
                Span::styled(preview, muted),
            ]));
        }
    }

    let mut lead = vec![header_guide.clone()];
    if context.anchor {
        lead.push(Span::styled(
            "◆ ",
            Style::default().add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(author_header(
        lead,
        &item.author_name,
        &item.author_handle,
        item.author_did.as_deref().unwrap_or(&item.author_handle),
        &time,
        width,
    ));

    lines.extend(body_lines(&item.text, &body_prefix, width));
    lines.extend(media_lines(
        &body_prefix,
        width,
        &item.images,
        &item.videos,
        item.external.as_ref(),
    ));
    if let Some(quote) = &item.quote {
        render_quote_lines(&mut lines, quote, width, &body_prefix);
    }
    if let Some(status) = &item.embed_status {
        lines.push(Line::from(vec![
            body_guide.clone(),
            Span::styled(status.clone(), muted),
        ]));
    }
    lines.push(engagement_line(item, &context.body_guide));

    let separator = if context.body_guide.trim().is_empty() {
        Line::from("")
    } else {
        Line::from(vec![
            Span::raw("  "),
            guide_span(context.body_guide.trim_end()),
        ])
    };
    (lines, separator)
}

#[cfg(test)]
pub(crate) fn engagement_summary(item: &FeedItem) -> String {
    engagement_line(item, "")
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect()
}

pub(crate) fn engagement_line(item: &FeedItem, prefix: &str) -> Line<'static> {
    let counter = |glyph: &str, count: u64, active: Option<Color>| {
        let style = match active {
            Some(color) => Style::default().fg(color).add_modifier(Modifier::BOLD),
            None if count == 0 => Style::default().fg(MUTED),
            None => Style::default(),
        };
        Span::styled(format!("{glyph} {}", compact_count(count)), style)
    };
    let liked = item.viewer_like.is_some();
    let reposted = item.viewer_repost.is_some();
    Line::from(vec![
        guide_span(prefix),
        counter("↩", item.reply_count, None),
        Span::raw("   "),
        counter("⟳", item.repost_count, reposted.then_some(Color::Green)),
        Span::raw("   "),
        counter(
            if liked { "♥" } else { "♡" },
            item.like_count,
            liked.then_some(Color::Red),
        ),
        Span::raw("   "),
        counter("❞", item.quote_count, None),
    ])
}

fn render_quote_lines(
    lines: &mut Vec<Line<'static>>,
    quote: &QuotePost,
    width: usize,
    body_prefix: &[Span<'static>],
) {
    let frame_style = Style::default().fg(FRAME);
    let mut lead = body_prefix.to_vec();
    lead.push(Span::styled("╭ ", frame_style));
    lines.push(author_header(
        lead,
        &quote.author_name,
        &quote.author_handle,
        &quote.author_handle,
        &compact_time(quote.indexed_at.as_deref()),
        width,
    ));
    let mut inner = body_prefix.to_vec();
    inner.push(Span::styled("│ ", frame_style));
    let mut quote_body = body_lines(&quote.text, &inner, width);
    quote_body.truncate(6);
    lines.extend(quote_body);
    lines.extend(media_lines(
        &inner,
        width,
        &quote.images,
        &quote.videos,
        quote.external.as_ref(),
    ));
    if let Some(nested) = &quote.nested_quote {
        let mut spans = inner.clone();
        spans.push(Span::styled(
            truncate_width(&format!("❞ {nested}"), width.saturating_sub(6)),
            Style::default().fg(MUTED),
        ));
        lines.push(Line::from(spans));
    }
    let mut close = body_prefix.to_vec();
    close.push(Span::styled("╰─", frame_style));
    lines.push(Line::from(close));
}

fn link_domain(uri: &str) -> String {
    uri.split("://")
        .nth(1)
        .unwrap_or(uri)
        .split('/')
        .next()
        .unwrap_or_default()
        .trim_start_matches("www.")
        .to_owned()
}

/// One line per attachment: alt text and card descriptions are truncated
/// here and shown in full in the media overlay / link picker.
fn media_lines(
    prefix: &[Span<'static>],
    width: usize,
    images: &[ImageRef],
    videos: &[crate::model::VideoRef],
    external: Option<&ExternalRef>,
) -> Vec<Line<'static>> {
    let prefix_width = prefix.iter().map(Span::width).sum::<usize>();
    let room = width.saturating_sub(prefix_width);
    let media_style = Style::default().fg(Color::LightMagenta);
    let mut lines = Vec::new();
    let mut push = |tag: String, detail: String, style: Style| {
        let text = if detail.is_empty() {
            tag
        } else {
            format!("{tag} {}", single_line(&detail))
        };
        let mut spans = prefix.to_vec();
        spans.push(Span::styled(truncate_width(&text, room), style));
        lines.push(Line::from(spans));
    };
    if !images.is_empty() {
        let tag = if images.len() == 1 {
            "[image]".to_owned()
        } else {
            format!("[{} images]", images.len())
        };
        let alt = images
            .iter()
            .find_map(|image| image.alt.clone())
            .unwrap_or_else(|| "(no alt text)".into());
        push(tag, alt, media_style);
    }
    if !videos.is_empty() {
        let alt = videos
            .iter()
            .find_map(|video| video.alt.clone())
            .unwrap_or_default();
        push("[video]".into(), alt, media_style);
    }
    if let Some(external) = external {
        let domain = link_domain(&external.uri);
        push(
            "[link]".into(),
            format!("{} · {domain}", external.title),
            Style::default().fg(LINK),
        );
    }
    lines
}

pub(crate) fn render_splash(frame: &mut Frame<'_>, handle: &str) {
    let area = frame.area();
    let lines = vec![
        Line::from(Span::styled(
            "at-tui",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("Connecting to Bluesky as @{handle}…"),
            Style::default().fg(MUTED),
        )),
    ];
    let top = area.height.saturating_sub(lines.len() as u16) / 2;
    let rect = Rect::new(area.x, area.y + top, area.width, lines.len() as u16);
    frame.render_widget(
        Paragraph::new(Text::from(lines)).alignment(ratatui::layout::Alignment::Center),
        rect,
    );
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
            author_following_uri: None,
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
        assert_eq!(engagement_summary(&item()), "↩ 2   ⟳ 3   ♡ 5   ❞ 7");

        let mut liked = item();
        liked.viewer_like = Some("at://did:plc:viewer/app.bsky.feed.like/1".into());
        assert_eq!(engagement_summary(&liked), "↩ 2   ⟳ 3   ♥ 5   ❞ 7");
    }

    #[test]
    fn styles_reposted_counter() {
        let mut item = item();
        item.viewer_repost = Some("at://did:plc:viewer/app.bsky.feed.repost/1".into());

        let line = engagement_line(&item, "");

        assert_eq!(line.spans[3].style.fg, Some(Color::Green));
        assert!(line.spans[3].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn styles_liked_counter() {
        let mut item = item();
        item.viewer_like = Some("at://did:plc:viewer/app.bsky.feed.like/1".into());

        let line = engagement_line(&item, "");

        assert_eq!(line.spans[5].content.as_ref(), "♥ 5");
        assert_eq!(line.spans[5].style.fg, Some(Color::Red));
        assert!(line.spans[5].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn renders_quote_in_box_drawing_frame() {
        let mut item = item();
        item.quote = Some(QuotePost {
            uri: "quote".into(),
            cid: None,
            author_name: "Bob".into(),
            author_handle: "bob.test".into(),
            text: "quoted words".into(),
            indexed_at: None,
            images: Vec::new(),
            videos: Vec::new(),
            external: None,
            links: Vec::new(),
            nested_quote: None,
        });

        let (lines, _) = render_item_lines(&item, 80, &RowContext::default());
        let text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("╭ Bob @bob.test"));
        assert!(text.contains("│ quoted words"));
        assert!(text.contains("╰─"));
        assert!(!text.contains("+--"));
    }

    fn plain(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn renders_repost_reason_text() {
        let mut item = item();
        item.reason = Some(FeedReason::Repost {
            by_name: "Alice".into(),
            by_handle: "alice.test".into(),
            indexed_at: None,
        });
        let (lines, _) = render_item_lines(&item, 80, &RowContext::default());
        assert!(plain(&lines).starts_with("⟳ reposted by Alice"));
    }

    #[test]
    fn selection_bar_runs_the_full_height_of_the_post() {
        let item = ViewItem::Post(Box::new(item_with_text("one\ntwo")));
        let lines = render_view_item_lines(&item, true, 40, &RowContext::default());
        let (separator, post) = lines.split_last().unwrap();
        assert!(post.iter().all(|line| line.spans[0].content == "▌ "));
        assert_eq!(separator.width(), 0);
    }

    #[test]
    fn header_right_aligns_time_and_shows_handle_once() {
        let line = author_header(
            Vec::new(),
            "alice.test",
            "alice.test",
            "did:plc:a",
            "5m",
            30,
        );
        let text = plain(std::slice::from_ref(&line));
        assert!(text.starts_with("@alice.test"), "{text}");
        assert!(text.ends_with(" 5m"));
        assert_eq!(line.width(), 30);
    }

    #[test]
    fn thread_guides_draw_siblings_and_last_children() {
        // anchor(0) -> a(1) -> a1(2); b(1)
        let guides = thread_guides(&[0, 1, 2, 1]);
        let headers = guides
            .iter()
            .map(|(header, _)| header.as_str())
            .collect::<Vec<_>>();
        assert_eq!(headers, vec!["", "├─ ", "│  └─ ", "└─ "]);
        // Body lines keep the column for later siblings and lead to replies.
        assert_eq!(guides[0].1, "│ ");
        assert_eq!(guides[1].1, "│  │ ");
        assert_eq!(guides[2].1, "│       ");
        assert_eq!(guides[3].1, "     ");
    }

    #[test]
    fn compacts_large_counts() {
        assert_eq!(compact_count(999), "999");
        assert_eq!(compact_count(1_000), "1K");
        assert_eq!(compact_count(9_231), "9.2K");
        assert_eq!(compact_count(86_027), "86K");
        assert_eq!(compact_count(1_250_000), "1.2M");
    }

    #[test]
    fn author_colour_is_stable_per_account() {
        assert_eq!(nick_color("did:plc:alice"), nick_color("did:plc:alice"));
        assert!(!NICK_COLORS.contains(&Color::Yellow));
        assert!(!NICK_COLORS.contains(&Color::Red));
    }

    #[test]
    fn status_line_is_compact_and_has_no_footer_controls() {
        let text = normal_status_text("alice.test", "Following", "Loaded");

        assert_eq!(text, " @alice.test  Following  Loaded ");
        assert!(!text.contains('/'));
        assert!(!text.contains("j/k"));
        assert!(!text.contains("replies"));
        assert!(!text.contains("img:"));
    }

    #[test]
    fn status_height_is_always_one_row() {
        assert_eq!(status_height(), 1);
    }

    #[test]
    fn renders_notification_row_with_unread_marker() {
        let item = NotificationItem {
            uri: "notification".into(),
            cid: "cid".into(),
            author_did: Some("did:plc:bob".into()),
            author_name: "Bob".into(),
            author_handle: "bob.test".into(),
            author_following_uri: None,
            reason: crate::model::NotificationReason::Reply,
            reason_subject: Some("at://did:plc:alice/app.bsky.feed.post/1".into()),
            text: "reply text".into(),
            indexed_at: "2026-05-22T00:00:00Z".into(),
            is_read: false,
            target: crate::model::NotificationTarget::Post {
                uri: "at://did:plc:alice/app.bsky.feed.post/1".into(),
            },
            others: vec!["Carol".into(), "Dan".into(), "Erin".into()],
            subject_text: None,
        };

        let lines = render_view_item_lines(
            &ViewItem::Notification(Box::new(item)),
            true,
            80,
            &RowContext::default(),
        );
        let text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            text.contains("● ↩ Bob, Carol and 2 others replied to you"),
            "{text}"
        );
        assert!(text.contains("reply text"));
    }

    #[test]
    fn renders_profile_header_summary() {
        let profile = ProfileSummary {
            did: "did:plc:alice".into(),
            handle: "alice.test".into(),
            display_name: "Alice".into(),
            description: Some("profile text".into()),
            avatar_url: None,
            banner_url: None,
            viewer_following: Some("at://did:plc:viewer/app.bsky.graph.follow/1".into()),
            followers_count: 1,
            follows_count: 2,
            posts_count: 3,
        };

        let lines = render_profile_header_lines(&profile, 80, "did:plc:viewer");
        let text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("Alice @alice.test"));
        assert!(text.contains("1 followers · 2 following · 3 posts"));
        assert!(text.contains("✓ following"));
    }

    #[test]
    fn omits_profile_follow_state_for_active_account() {
        let profile = ProfileSummary {
            did: "did:plc:alice".into(),
            handle: "alice.test".into(),
            display_name: "Alice".into(),
            description: None,
            avatar_url: None,
            banner_url: None,
            viewer_following: None,
            followers_count: 1,
            follows_count: 2,
            posts_count: 3,
        };

        let lines = render_profile_header_lines(&profile, 80, "did:plc:alice");
        let text = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("1 followers · 2 following · 3 posts"));
        assert!(!text.contains("not following"));
        assert!(!text.contains("✓ following"));
    }

    #[test]
    fn wraps_and_styles_media_and_link_summaries() {
        let images = vec![ImageRef {
            thumb_url: "https://example.com/thumb.jpg".into(),
            fullsize_url: Some("https://example.com/full.jpg".into()),
            alt: Some("a long image alt text that wraps cleanly".into()),
        }];
        let videos = vec![crate::model::VideoRef {
            playlist_url: "https://example.com/video.m3u8".into(),
            thumb_url: None,
            alt: Some("a long video alt text that wraps cleanly".into()),
            cid: None,
            aspect_ratio: None,
        }];
        let external = ExternalRef {
            uri: "https://example.com/article".into(),
            title: "Article title".into(),
            description: Some("a long external description that wraps cleanly".into()),
            thumb_url: None,
        };
        let lines = media_lines(&[Span::raw("  ")], 28, &images, &videos, Some(&external));

        // One line per attachment; long alt text is truncated, not wrapped.
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| line.width() <= 28));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::LightMagenta))
        }));
        assert!(
            lines
                .iter()
                .any(|line| { line.spans.iter().any(|span| span.style.fg == Some(LINK)) })
        );
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

        let _ = visible_feed_lines_at(&mut view, 80, 10, 100);
        let first_builds = view.layout_cache.builds;
        let _ = visible_feed_lines_at(&mut view, 80, 10, 100);

        assert_eq!(first_builds, 1);
        assert_eq!(view.layout_cache.builds, first_builds);

        let _ = visible_feed_lines_at(&mut view, 40, 10, 100);
        assert_eq!(view.layout_cache.builds, first_builds + 1);

        // Relative timestamps age: a new minute rebuilds the rows.
        let _ = visible_feed_lines_at(&mut view, 40, 10, 101);
        assert_eq!(view.layout_cache.builds, first_builds + 2);
    }
}
