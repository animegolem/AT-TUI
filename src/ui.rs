use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::{
    app::{App, InputMode},
    model::{
        ExternalRef, FeedItem, FeedReason, ImageRef, QuotePost, ReplyContext, ReplyParentStatus,
        compact_time,
    },
    navigation::ViewState,
};

pub fn render(frame: &mut Frame<'_>, app: &mut App) {
    let area = frame.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(frame, chunks[0], app);
    render_body(frame, chunks[1], app);
    render_status(frame, chunks[2], app);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let text = format!(
        " {} | {} ",
        app.nav.breadcrumb(),
        app.home_feed_prefs.status_label()
    );
    frame.render_widget(
        Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Cyan)),
        area,
    );
}

fn render_body(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    if area.width >= 112 {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
            .split(area);
        render_feed(frame, columns[0], app);
        render_preview(frame, columns[1], app);
    } else {
        render_feed(frame, area, app);
    }
}

fn render_feed(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let view = app.nav.current_mut();
    let title = if view.loading {
        format!("{} [loading]", view.title)
    } else {
        view.title.clone()
    };
    let block = Block::default().title(title).borders(Borders::ALL);
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
    ensure_selected_rendered(view, width, available_lines);

    if view.items.is_empty() {
        return vec![Line::from("No posts in this view.")];
    }

    let mut lines = Vec::new();
    let mut used = 0usize;
    for (index, item) in view.items.iter().enumerate().skip(view.scroll) {
        if used >= available_lines {
            break;
        }

        let selected = index == view.selected;
        let item_lines = render_item_lines(item, selected, width);

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
        && rendered_height(&view.items[view.scroll..=view.selected], width) > available_lines
    {
        view.scroll += 1;
    }
}

fn rendered_height(items: &[FeedItem], width: usize) -> usize {
    items
        .iter()
        .map(|item| render_item_lines(item, false, width).len())
        .sum()
}

fn render_preview(frame: &mut Frame<'_>, area: Rect, app: &mut App) {
    let selected = app.nav.current().selected_item().cloned();
    let Some(item) = selected else {
        frame.render_widget(
            Paragraph::new("No selection")
                .block(Block::default().title("Preview").borders(Borders::ALL)),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(area);

    let mut lines = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        format!("{} @{}", item.author_name, item.author_handle),
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )]));
    lines.push(Line::from(engagement_summary(&item)));
    if let Some(reason) = &item.reason {
        lines.push(Line::from(reason_text(reason)));
    }
    if let Some(reply) = &item.reply {
        lines.extend(reply_preview_lines(
            reply,
            chunks[0].width.saturating_sub(4) as usize,
            "",
        ));
    }
    lines.push(Line::from(""));
    for line in wrap_text(&item.text, chunks[0].width.saturating_sub(4) as usize) {
        lines.push(Line::from(line));
    }
    if let Some(quote) = &item.quote {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![Span::styled(
            format!("Quoted: {} @{}", quote.author_name, quote.author_handle),
            Style::default().fg(Color::Yellow),
        )]));
        for line in wrap_text(&quote.text, chunks[0].width.saturating_sub(4) as usize) {
            lines.push(Line::from(format!("> {line}")));
        }
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .block(Block::default().title("Selected").borders(Borders::ALL))
            .wrap(Wrap { trim: false }),
        chunks[0],
    );
    app.media.render_first_image(frame, chunks[1], &item);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let status = match &app.input_mode {
        InputMode::Normal => format!(
            " {} | j/k move, l/Enter replies, h back, o quote, / search, r reload, q quit | images:{} ",
            app.status,
            app.media.protocol_name()
        ),
        InputMode::Search { buffer } => format!(" /{buffer} "),
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(Color::Black).bg(Color::Gray)),
        area,
    );
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
    render_media_summary(lines, &quote_prefix, &quote.images, quote.external.as_ref());
    if let Some(nested) = &quote.nested_quote {
        lines.push(Line::from(format!("{quote_prefix}nested quote: {nested}")));
    }
}

fn render_media_summary(
    lines: &mut Vec<Line<'static>>,
    prefix: &str,
    images: &[ImageRef],
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
            external: None,
            quote: None,
            reason: None,
            reply: None,
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
}
