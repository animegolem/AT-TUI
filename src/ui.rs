use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Wrap},
};

use crate::{
    app::{
        App, ComposerKind, ComposerState, InputMode, MenuSection, Overlay, normal_key_help_lines,
    },
    media::{PreviewImage, PreviewMedia},
    model::{
        ExternalRef, FeedItem, FeedReason, ImageRef, NotificationItem, NotificationTarget,
        ProfileSummary, QuotePost, ReplyContext, ReplyParentStatus, ViewItem, compact_time,
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
    let title = if view.loading {
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
    let list_available = available_lines.saturating_sub(profile_lines.len());
    let mut lines = profile_lines;
    lines.extend(visible_feed_lines(view, inner_width, list_available));

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
            selected: render_view_item_lines(item, true, width),
            unselected: render_view_item_lines(item, false, width),
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
            .map(|item| render_view_item_lines(item, false, width).len())
            .sum();
    }

    view.layout_cache.items[start..=end]
        .iter()
        .map(|item| item.unselected.len())
        .sum()
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App) {
    frame.render_widget(
        Paragraph::new(" ".repeat(area.width as usize)).style(Style::default().bg(Color::DarkGray)),
        area,
    );
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
        Overlay::Menu(state) => render_menu_overlay(frame, area, app, state.section),
        Overlay::Media(state) => {
            if let Some(media) = state.selected_media().cloned() {
                render_media_overlay(frame, area, app, &media, state.selected, state.media.len());
            }
        }
        Overlay::Links(state) => render_link_overlay(frame, area, state.links, state.selected),
        Overlay::Composer(state) => render_composer_overlay(frame, area, state),
        Overlay::ConfirmDelete(_) => render_confirm_delete_overlay(frame, area),
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

fn render_menu_overlay(frame: &mut Frame<'_>, area: Rect, app: &App, selected: MenuSection) {
    let area = centered_rect(86, 82, area);
    frame.render_widget(Clear, area);

    let mut lines = Vec::new();
    lines.push(section_line(MenuSection::Keys, selected));
    lines.extend(normal_key_help_lines().into_iter().map(Line::from));
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
    lines.push(Line::from(
        "  Tab next account · Shift-Tab previous account",
    ));
    lines.push(Line::from(""));

    lines.push(section_line(MenuSection::Feeds, selected));
    for (index, feed) in app.feeds.iter().take(8).enumerate() {
        let marker = if index == app.active_feed { "*" } else { " " };
        lines.push(Line::from(format!("  {marker} {}", feed.label)));
    }
    lines.push(Line::from(
        "  Tab next feed · Shift-Tab previous feed · [ and ] also switch feeds",
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
    let count = crate::app::post_grapheme_count(&state.buffer);
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

fn status_left_line_inner(app: &App, include_transient_status: bool) -> Line<'static> {
    let mut spans = vec![
        segment(
            format!(" @{} ", app.client.session().handle),
            Color::Black,
            Color::Cyan,
        ),
        segment(
            format!(" {} ", current_location_label(app)),
            Color::Black,
            Color::Yellow,
        ),
    ];

    if app.pending_new_count() > 0 {
        spans.push(segment(
            format!(" ↑ {} new ", app.pending_new_count()),
            Color::Black,
            Color::Green,
        ));
    }

    if app.unread_notifications > 0 {
        spans.push(segment(
            format!(" ! {} ", app.unread_notifications),
            Color::Black,
            Color::LightRed,
        ));
    }

    if app.is_offline() {
        spans.push(segment(" ⚠ offline ".to_owned(), Color::Black, Color::Red));
    }

    if app.has_pending_tasks() {
        spans.push(segment(" … ".to_owned(), Color::Black, Color::Magenta));
    }

    let status = if include_transient_status {
        status_text(app)
    } else {
        String::new()
    };
    if !status.is_empty() {
        spans.push(Span::styled(
            format!(" {status} "),
            Style::default().fg(Color::Gray),
        ));
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
    Line::from(vec![segment(
        format!(" {} ", app.current_position_label()),
        Color::Black,
        Color::LightBlue,
    )])
}

pub(crate) fn current_location_label(app: &App) -> String {
    if app.nav.depth() > 1 {
        app.nav.current().title.clone()
    } else {
        active_feed_label(app).to_owned()
    }
}

pub(crate) fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

fn segment(text: String, fg: Color, bg: Color) -> Span<'static> {
    Span::styled(
        text,
        Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
    )
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

fn render_profile_header_lines(
    profile: &ProfileSummary,
    width: usize,
    active_did: &str,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("{} @{}", profile.display_name, profile.handle),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    if let Some(description) = &profile.description {
        for line in wrap_text(description, width).into_iter().take(3) {
            lines.push(Line::from(line));
        }
    }
    let follow_state = if !profile.did.is_empty() && profile.did != active_did {
        if profile.viewer_following.is_some() {
            "  · following"
        } else {
            "  · not following"
        }
    } else {
        ""
    };
    lines.push(Line::from(vec![Span::styled(
        format!(
            "{} followers  {} following  {} posts{}",
            profile.followers_count, profile.follows_count, profile.posts_count, follow_state
        ),
        Style::default().fg(Color::DarkGray),
    )]));
    lines.push(Line::from(""));
    lines
}

fn render_view_item_lines(item: &ViewItem, selected: bool, width: usize) -> Vec<Line<'static>> {
    match item {
        ViewItem::Post(item) => render_item_lines(item, selected, width),
        ViewItem::Notification(item) => render_notification_lines(item, selected, width),
    }
}

fn render_notification_lines(
    item: &NotificationItem,
    selected: bool,
    width: usize,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let marker = if selected { ">" } else { " " };
    let unread = if item.is_read { " " } else { "●" };
    let time = compact_time(Some(&item.indexed_at));
    let header_style = if selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else if item.is_read {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    };

    lines.push(Line::from(vec![Span::styled(
        format!(
            "{marker} {unread} {} @{} {} {}",
            item.author_name,
            item.author_handle,
            item.reason.label(),
            time
        ),
        header_style,
    )]));

    let body_prefix = "  ";
    if !item.text.trim().is_empty() {
        for line in wrap_text(&item.text, width.saturating_sub(body_prefix.len()).max(10))
            .into_iter()
            .take(3)
        {
            lines.push(Line::from(format!("{body_prefix}{line}")));
        }
    }

    if let Some(subject) = &item.reason_subject {
        lines.push(Line::from(vec![Span::styled(
            format!("{body_prefix}subject: {subject}"),
            Style::default().fg(Color::DarkGray),
        )]));
    } else if matches!(item.target, NotificationTarget::None) {
        lines.push(Line::from(vec![Span::styled(
            format!("{body_prefix}[no openable target]"),
            Style::default().fg(Color::DarkGray),
        )]));
    }

    lines.push(Line::from(""));
    lines
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
        width,
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

    lines.push(engagement_line(item, &body_prefix));
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

#[cfg(test)]
pub(crate) fn engagement_summary(item: &FeedItem) -> String {
    let like_symbol = if item.viewer_like.is_some() {
        "♥"
    } else {
        "♡"
    };
    format!(
        "↩ {}  ⟳ {}  {} {}  ❞ {}",
        item.reply_count, item.repost_count, like_symbol, item.like_count, item.quote_count
    )
}

pub(crate) fn engagement_line(item: &FeedItem, prefix: &str) -> Line<'static> {
    let repost_style = if item.viewer_repost.is_some() {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let like_symbol = if item.viewer_like.is_some() {
        "♥"
    } else {
        "♡"
    };
    let like_style = if item.viewer_like.is_some() {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Line::from(vec![
        Span::raw(prefix.to_owned()),
        Span::raw(format!("↩ {}  ", item.reply_count)),
        Span::styled(format!("⟳ {}", item.repost_count), repost_style),
        Span::raw("  "),
        Span::styled(format!("{} {}", like_symbol, item.like_count), like_style),
        Span::raw(format!("  ❞ {}", item.quote_count)),
    ])
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
        width,
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
    width: usize,
    images: &[ImageRef],
    videos: &[crate::model::VideoRef],
    external: Option<&ExternalRef>,
) {
    let content_width = width.saturating_sub(prefix.len()).max(10);
    let media_style = Style::default().fg(Color::LightMagenta);
    let link_style = Style::default().fg(Color::LightCyan);

    if !images.is_empty() {
        let label = if images.len() == 1 { "image" } else { "images" };
        let alt = images
            .first()
            .and_then(|image| image.alt.as_ref())
            .map(|alt| format!(": {alt}"))
            .unwrap_or_default();
        push_wrapped_summary(
            lines,
            prefix,
            &format!("[{} {label}{alt}]", images.len()),
            content_width,
            media_style,
        );
    }
    if !videos.is_empty() {
        let label = if videos.len() == 1 { "video" } else { "videos" };
        let alt = videos
            .first()
            .and_then(|video| video.alt.as_ref())
            .map(|alt| format!(": {alt}"))
            .unwrap_or_default();
        push_wrapped_summary(
            lines,
            prefix,
            &format!("[{} {label}{alt}]", videos.len()),
            content_width,
            media_style,
        );
    }
    if let Some(external) = external {
        let description = external
            .description
            .as_ref()
            .map(|description| format!(" - {description}"))
            .unwrap_or_default();
        push_wrapped_summary(
            lines,
            prefix,
            &format!("[link] {}{}", external.title, description),
            content_width,
            link_style,
        );
    }
}

fn push_wrapped_summary(
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    text: &str,
    width: usize,
    style: Style,
) {
    for line in wrap_text(text, width) {
        lines.push(Line::from(vec![
            Span::raw(prefix.to_owned()),
            Span::styled(line, style),
        ]));
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
        assert_eq!(engagement_summary(&item()), "↩ 2  ⟳ 3  ♡ 5  ❞ 7");

        let mut liked = item();
        liked.viewer_like = Some("at://did:plc:viewer/app.bsky.feed.like/1".into());
        assert_eq!(engagement_summary(&liked), "↩ 2  ⟳ 3  ♥ 5  ❞ 7");
    }

    #[test]
    fn styles_reposted_counter() {
        let mut item = item();
        item.viewer_repost = Some("at://did:plc:viewer/app.bsky.feed.repost/1".into());

        let line = engagement_line(&item, "");

        assert_eq!(line.spans[2].style.fg, Some(Color::Green));
        assert!(line.spans[2].style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn styles_liked_counter() {
        let mut item = item();
        item.viewer_like = Some("at://did:plc:viewer/app.bsky.feed.like/1".into());

        let line = engagement_line(&item, "");

        assert_eq!(line.spans[4].content.as_ref(), "♥ 5");
        assert_eq!(line.spans[4].style.fg, Some(Color::Red));
        assert!(line.spans[4].style.add_modifier.contains(Modifier::BOLD));
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
            target: NotificationTarget::Post {
                uri: "at://did:plc:alice/app.bsky.feed.post/1".into(),
            },
        };

        let lines = render_view_item_lines(&ViewItem::Notification(Box::new(item)), true, 80);
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

        assert!(text.contains("● Bob @bob.test replied to you"));
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
        assert!(text.contains("1 followers  2 following  3 posts  · following"));
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

        assert!(text.contains("1 followers  2 following  3 posts"));
        assert!(!text.contains("not following"));
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
        let mut lines = Vec::new();

        render_media_summary(&mut lines, "  ", 28, &images, &videos, Some(&external));

        assert!(lines.len() > 3);
        assert!(lines.iter().all(|line| line_width(line) <= 28));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::LightMagenta))
        }));
        assert!(lines.iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.style.fg == Some(Color::LightCyan))
        }));
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
