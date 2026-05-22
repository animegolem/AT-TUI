use chrono::{DateTime, Utc};
use linkify::{LinkFinder, LinkKind};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedItem {
    pub uri: String,
    pub cid: Option<String>,
    pub viewer_like: Option<String>,
    pub viewer_repost: Option<String>,
    pub author_did: Option<String>,
    pub author_name: String,
    pub author_handle: String,
    pub author_following: Option<bool>,
    pub avatar_url: Option<String>,
    pub text: String,
    pub indexed_at: Option<String>,
    pub reply_count: u64,
    pub repost_count: u64,
    pub like_count: u64,
    pub quote_count: u64,
    pub images: Vec<ImageRef>,
    pub videos: Vec<VideoRef>,
    pub external: Option<ExternalRef>,
    pub links: Vec<LinkRef>,
    pub quote: Option<QuotePost>,
    pub reason: Option<FeedReason>,
    pub reply: Option<ReplyContext>,
    pub reply_root: Option<PostRef>,
    pub embed_status: Option<String>,
    pub depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedReason {
    Repost {
        by_name: String,
        by_handle: String,
        indexed_at: Option<String>,
    },
    Pin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyContext {
    pub root_uri: String,
    pub parent_uri: String,
    pub parent_author_name: String,
    pub parent_author_handle: String,
    pub parent_text: String,
    pub grandparent_author_handle: Option<String>,
    pub parent_status: Option<ReplyParentStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplyParentStatus {
    NotFound,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotePost {
    pub uri: String,
    pub cid: Option<String>,
    pub author_name: String,
    pub author_handle: String,
    pub text: String,
    pub indexed_at: Option<String>,
    pub images: Vec<ImageRef>,
    pub videos: Vec<VideoRef>,
    pub external: Option<ExternalRef>,
    pub links: Vec<LinkRef>,
    pub nested_quote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostRef {
    pub uri: String,
    pub cid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRef {
    pub thumb_url: String,
    pub fullsize_url: Option<String>,
    pub alt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoRef {
    pub playlist_url: String,
    pub thumb_url: Option<String>,
    pub alt: Option<String>,
    pub cid: Option<String>,
    pub aspect_ratio: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalRef {
    pub uri: String,
    pub title: String,
    pub description: Option<String>,
    pub thumb_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkRef {
    pub uri: String,
    pub label: String,
    pub source: LinkSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkSource {
    External,
    Text,
    QuoteExternal,
    QuoteText,
}

impl LinkSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::External => "card",
            Self::Text => "text",
            Self::QuoteExternal => "quote card",
            Self::QuoteText => "quote text",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedSource {
    pub label: String,
    pub kind: FeedSourceKind,
}

impl FeedSource {
    pub fn home() -> Self {
        Self {
            label: "Following".into(),
            kind: FeedSourceKind::Home,
        }
    }

    pub fn author(handle: impl Into<String>, did: impl Into<String>) -> Self {
        Self {
            label: "Your Posts".into(),
            kind: FeedSourceKind::Author {
                handle: handle.into(),
                did: did.into(),
            },
        }
    }

    pub fn uri(&self) -> Option<&str> {
        match &self.kind {
            FeedSourceKind::Home => None,
            FeedSourceKind::Author { .. } => None,
            FeedSourceKind::Generator { uri } => Some(uri),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FeedSourceKind {
    Home,
    Author { handle: String, did: String },
    Generator { uri: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HomeFeedPrefs {
    pub hide_replies: bool,
    pub hide_replies_by_unfollowed: bool,
    pub hide_replies_by_like_count: Option<u64>,
    pub hide_reposts: bool,
    pub hide_quote_posts: bool,
}

impl HomeFeedPrefs {
    pub fn from_preferences_response(root: &Value) -> Self {
        root.get("preferences")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|pref| {
                string_field(pref, "$type")
                    .is_some_and(|value| value == "app.bsky.actor.defs#feedViewPref")
                    && string_field(pref, "feed").is_some_and(|value| value == "home")
            })
            .map(Self::from_feed_view_pref)
            .unwrap_or_default()
    }

    pub fn status_label(&self) -> String {
        let replies = if self.hide_replies {
            "off"
        } else if self.hide_replies_by_unfollowed || self.hide_replies_by_like_count.is_some() {
            "filtered"
        } else {
            "on"
        };
        let reposts = if self.hide_reposts { "off" } else { "on" };
        let quotes = if self.hide_quote_posts { "off" } else { "on" };
        format!("Following · replies:{replies} reposts:{reposts} quotes:{quotes}")
    }

    fn from_feed_view_pref(pref: &Value) -> Self {
        Self {
            hide_replies: bool_field(pref, "hideReplies").unwrap_or(false),
            hide_replies_by_unfollowed: bool_field(pref, "hideRepliesByUnfollowed")
                .unwrap_or(false),
            hide_replies_by_like_count: number_field_opt(pref, "hideRepliesByLikeCount"),
            hide_reposts: bool_field(pref, "hideReposts").unwrap_or(false),
            hide_quote_posts: bool_field(pref, "hideQuotePosts").unwrap_or(false),
        }
    }

    pub fn allows(&self, item: &FeedItem) -> bool {
        if self.hide_reposts && matches!(item.reason, Some(FeedReason::Repost { .. })) {
            return false;
        }

        if self.hide_quote_posts && item.quote.is_some() {
            return false;
        }

        if item.reply.is_some() {
            if self.hide_replies {
                return false;
            }

            if self.hide_replies_by_unfollowed && item.author_following == Some(false) {
                return false;
            }

            if let Some(min_likes) = self.hide_replies_by_like_count
                && item.like_count < min_likes
            {
                return false;
            }
        }

        true
    }
}

pub fn feed_sources_from_preferences(root: &Value) -> Vec<FeedSource> {
    let mut sources = vec![FeedSource::home()];
    let mut seen = std::collections::HashSet::new();

    for uri in saved_feed_uris(root) {
        if seen.insert(uri.clone()) {
            sources.push(FeedSource {
                label: short_feed_label(&uri),
                kind: FeedSourceKind::Generator { uri },
            });
        }
    }

    sources
}

pub fn feed_sources_for_account(root: &Value, handle: &str, did: &str) -> Vec<FeedSource> {
    let mut sources = vec![FeedSource::home(), FeedSource::author(handle, did)];
    let mut seen = std::collections::HashSet::new();

    for uri in saved_feed_uris(root) {
        if seen.insert(uri.clone()) {
            sources.push(FeedSource {
                label: short_feed_label(&uri),
                kind: FeedSourceKind::Generator { uri },
            });
        }
    }

    sources
}

pub fn timeline_items(root: &Value, prefs: &HomeFeedPrefs) -> (Vec<FeedItem>, Option<String>) {
    let items = root
        .get("feed")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(feed_item_from_feed_entry)
        .filter(|item| prefs.allows(item))
        .collect();
    let cursor = string_field(root, "cursor");
    (items, cursor)
}

fn saved_feed_uris(root: &Value) -> Vec<String> {
    let mut uris = Vec::new();
    for pref in root
        .get("preferences")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match string_field(pref, "$type").as_deref() {
            Some("app.bsky.actor.defs#savedFeedsPrefV2") => {
                if let Some(items) = pref.get("items").and_then(Value::as_array) {
                    for item in items {
                        if string_field(item, "type").as_deref() == Some("feed")
                            && let Some(uri) = string_field(item, "value")
                        {
                            uris.push(uri);
                        }
                    }
                }
            }
            Some("app.bsky.actor.defs#savedFeedsPref") => {
                for field in ["pinned", "saved"] {
                    if let Some(items) = pref.get(field).and_then(Value::as_array) {
                        uris.extend(
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|uri| is_feed_generator_uri(uri))
                                .map(ToOwned::to_owned),
                        );
                    }
                }
            }
            _ => {}
        }
    }
    uris
}

fn is_feed_generator_uri(uri: &str) -> bool {
    uri.contains("/app.bsky.feed.generator/")
}

fn short_feed_label(uri: &str) -> String {
    uri.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(uri)
        .to_owned()
}

pub fn thread_items(root: &Value) -> Vec<FeedItem> {
    let mut items = Vec::new();
    if let Some(thread) = root.get("thread") {
        let selected_depth = flatten_thread_parents(thread, &mut items);
        flatten_thread(thread, selected_depth, &mut items);
    }
    items
}

pub fn feed_item_from_feed_entry(entry: &Value) -> Option<FeedItem> {
    let post = entry.get("post")?;
    let mut item = feed_item_from_post(post, 0);
    item.reason = parse_reason(entry.get("reason"));
    item.reply = parse_reply_context(entry.get("reply"));
    Some(item)
}

pub fn feed_item_from_post(post: &Value, depth: usize) -> FeedItem {
    let author = post.get("author").unwrap_or(&Value::Null);
    let mut images = Vec::new();
    let mut videos = Vec::new();
    let mut external = None;
    let mut quote = None;
    let mut embed_status = None;

    if let Some(embed) = post.get("embed") {
        parse_embed(
            embed,
            &mut images,
            &mut videos,
            &mut external,
            &mut quote,
            &mut embed_status,
        );
    }

    FeedItem {
        uri: string_field(post, "uri").unwrap_or_default(),
        cid: string_field(post, "cid"),
        viewer_like: viewer_record_uri(post, "like"),
        viewer_repost: viewer_record_uri(post, "repost"),
        author_did: string_field(author, "did"),
        author_name: display_name(author),
        author_handle: string_field(author, "handle").unwrap_or_else(|| "unknown".into()),
        author_following: author_following(author),
        avatar_url: string_field(author, "avatar"),
        text: post_text(post),
        indexed_at: string_field(post, "indexedAt")
            .or_else(|| record_text_field(post, "createdAt")),
        reply_count: number_field(post, "replyCount"),
        repost_count: number_field(post, "repostCount"),
        like_count: number_field(post, "likeCount"),
        quote_count: number_field(post, "quoteCount"),
        images,
        videos,
        external,
        links: post_links(post, LinkSource::Text),
        quote,
        reason: None,
        reply: None,
        reply_root: post_reply_root(post),
        embed_status,
        depth,
    }
}

pub fn feed_item_from_quote(quote: QuotePost, depth: usize) -> FeedItem {
    FeedItem {
        uri: quote.uri,
        cid: quote.cid,
        viewer_like: None,
        viewer_repost: None,
        author_did: None,
        author_name: quote.author_name,
        author_handle: quote.author_handle,
        author_following: None,
        avatar_url: None,
        text: quote.text,
        indexed_at: quote.indexed_at,
        reply_count: 0,
        repost_count: 0,
        like_count: 0,
        quote_count: 0,
        images: quote.images,
        videos: quote.videos,
        external: quote.external,
        links: quote.links,
        quote: None,
        reason: None,
        reply: None,
        reply_root: None,
        embed_status: quote.nested_quote,
        depth,
    }
}

fn parse_reason(reason: Option<&Value>) -> Option<FeedReason> {
    let reason = reason?;
    let reason_type = string_field(reason, "$type").unwrap_or_default();
    if reason_type.ends_with("#reasonRepost") || reason.get("by").is_some() {
        let by = reason.get("by").unwrap_or(&Value::Null);
        return Some(FeedReason::Repost {
            by_name: display_name(by),
            by_handle: string_field(by, "handle").unwrap_or_else(|| "unknown".into()),
            indexed_at: string_field(reason, "indexedAt"),
        });
    }

    if reason_type.ends_with("#reasonPin") {
        return Some(FeedReason::Pin);
    }

    None
}

fn parse_reply_context(reply: Option<&Value>) -> Option<ReplyContext> {
    let reply = reply?;
    let root = reply.get("root").unwrap_or(&Value::Null);
    let parent = reply.get("parent")?;
    let root_uri = post_union_uri(root).unwrap_or_default();
    let parent_summary = reply_post_summary(parent);
    let grandparent_author_handle = reply
        .get("grandparentAuthor")
        .and_then(|author| string_field(author, "handle"));

    Some(ReplyContext {
        root_uri,
        parent_uri: parent_summary.uri,
        parent_author_name: parent_summary.author_name,
        parent_author_handle: parent_summary.author_handle,
        parent_text: parent_summary.text,
        grandparent_author_handle,
        parent_status: parent_summary.status,
    })
}

struct ReplyPostSummary {
    uri: String,
    author_name: String,
    author_handle: String,
    text: String,
    status: Option<ReplyParentStatus>,
}

fn reply_post_summary(value: &Value) -> ReplyPostSummary {
    if value
        .get("notFound")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return ReplyPostSummary {
            uri: string_field(value, "uri").unwrap_or_default(),
            author_name: "Post not found".into(),
            author_handle: "not-found".into(),
            text: "[post not found]".into(),
            status: Some(ReplyParentStatus::NotFound),
        };
    }

    if value
        .get("blocked")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return ReplyPostSummary {
            uri: string_field(value, "uri").unwrap_or_default(),
            author_name: "Blocked post".into(),
            author_handle: "blocked".into(),
            text: "[blocked post]".into(),
            status: Some(ReplyParentStatus::Blocked),
        };
    }

    let author = value.get("author").unwrap_or(&Value::Null);
    ReplyPostSummary {
        uri: string_field(value, "uri").unwrap_or_default(),
        author_name: display_name(author),
        author_handle: string_field(author, "handle").unwrap_or_else(|| "unknown".into()),
        text: post_text(value),
        status: None,
    }
}

fn post_union_uri(value: &Value) -> Option<String> {
    string_field(value, "uri")
}

fn flatten_thread_parents(node: &Value, items: &mut Vec<FeedItem>) -> usize {
    let Some(parent) = node.get("parent") else {
        return 0;
    };

    let depth = flatten_thread_parents(parent, items);
    if let Some(post) = thread_post(parent) {
        items.push(feed_item_from_post(post, depth));
        depth + 1
    } else {
        depth
    }
}

fn flatten_thread(node: &Value, depth: usize, items: &mut Vec<FeedItem>) {
    let Some(post) = thread_post(node) else {
        return;
    };

    items.push(feed_item_from_post(post, depth));

    if let Some(replies) = node.get("replies").and_then(Value::as_array) {
        for reply in replies {
            flatten_thread(reply, depth + 1, items);
        }
    }
}

fn thread_post(node: &Value) -> Option<&Value> {
    let node_type = string_field(node, "$type").unwrap_or_default();
    if node_type.ends_with("#notFoundPost") {
        return None;
    }
    if node_type.ends_with("#blockedPost") {
        return None;
    }

    node.get("post")
}

fn parse_embed(
    embed: &Value,
    images: &mut Vec<ImageRef>,
    videos: &mut Vec<VideoRef>,
    external: &mut Option<ExternalRef>,
    quote: &mut Option<QuotePost>,
    embed_status: &mut Option<String>,
) {
    let embed_type = string_field(embed, "$type").unwrap_or_default();
    match embed_type.as_str() {
        "app.bsky.embed.images#view" => images.extend(parse_images(embed)),
        "app.bsky.embed.external#view" => *external = parse_external(embed),
        "app.bsky.embed.record#view" => {
            if let Some((record_quote, status)) = parse_record_embed(embed.get("record")) {
                *quote = record_quote;
                *embed_status = status;
            }
        }
        "app.bsky.embed.recordWithMedia#view" => {
            if let Some(media) = embed.get("media") {
                parse_embed(media, images, videos, external, quote, embed_status);
            }
            if let Some((record_quote, status)) = parse_record_embed(embed.get("record")) {
                *quote = record_quote;
                *embed_status = status;
            }
        }
        "app.bsky.embed.video#view" => {
            if let Some(video) = parse_video(embed) {
                videos.push(video);
            } else {
                *embed_status = Some("[video embed unavailable]".into());
            }
        }
        _ if !embed_type.is_empty() => {
            *embed_status = Some(format!("[unsupported embed: {embed_type}]"));
        }
        _ => {}
    }
}

fn parse_record_embed(record: Option<&Value>) -> Option<(Option<QuotePost>, Option<String>)> {
    let record = record?;
    let record_type = string_field(record, "$type").unwrap_or_default();

    if record_type.ends_with("#viewRecord") || record.get("author").is_some() {
        return Some((Some(quote_from_record(record)), None));
    }

    if record_type.ends_with("#viewNotFound") {
        return Some((None, Some("[quoted post not found]".into())));
    }

    if record_type.ends_with("#viewBlocked") {
        return Some((None, Some("[quoted post blocked]".into())));
    }

    if let Some(inner) = record.get("record") {
        return parse_record_embed(Some(inner));
    }

    if record_type.is_empty() {
        None
    } else {
        Some((
            None,
            Some(format!("[unsupported quoted record: {record_type}]")),
        ))
    }
}

fn quote_from_record(record: &Value) -> QuotePost {
    let author = record.get("author").unwrap_or(&Value::Null);
    let mut images = Vec::new();
    let mut videos = Vec::new();
    let mut external = None;
    let mut nested_quote = None;
    let mut status = None;

    if let Some(embed) = record
        .get("embeds")
        .and_then(Value::as_array)
        .and_then(|v| v.first())
    {
        let mut quote = None;
        parse_embed(
            embed,
            &mut images,
            &mut videos,
            &mut external,
            &mut quote,
            &mut status,
        );
        nested_quote =
            quote.map(|quoted| format!("@{}: {}", quoted.author_handle, first_line(&quoted.text)));
    } else if let Some(embed) = record.get("embed") {
        let mut quote = None;
        parse_embed(
            embed,
            &mut images,
            &mut videos,
            &mut external,
            &mut quote,
            &mut status,
        );
        nested_quote =
            quote.map(|quoted| format!("@{}: {}", quoted.author_handle, first_line(&quoted.text)));
    }

    QuotePost {
        uri: string_field(record, "uri").unwrap_or_default(),
        cid: string_field(record, "cid"),
        author_name: display_name(author),
        author_handle: string_field(author, "handle").unwrap_or_else(|| "unknown".into()),
        text: record_value_text(record),
        indexed_at: string_field(record, "indexedAt")
            .or_else(|| record_value_field(record, "createdAt")),
        images,
        videos,
        external,
        links: quote_record_links(record),
        nested_quote: nested_quote.or(status),
    }
}

pub fn item_links(item: &FeedItem) -> Vec<LinkRef> {
    let mut links = Vec::new();
    if let Some(external) = &item.external
        && !external.uri.is_empty()
    {
        links.push(LinkRef {
            uri: external.uri.clone(),
            label: external.title.clone(),
            source: LinkSource::External,
        });
    }
    links.extend(item.links.iter().cloned());
    if let Some(quote) = &item.quote {
        if let Some(external) = &quote.external
            && !external.uri.is_empty()
        {
            links.push(LinkRef {
                uri: external.uri.clone(),
                label: external.title.clone(),
                source: LinkSource::QuoteExternal,
            });
        }
        links.extend(quote.links.iter().map(|link| LinkRef {
            uri: link.uri.clone(),
            label: link.label.clone(),
            source: LinkSource::QuoteText,
        }));
    }

    dedupe_links(links)
}

fn parse_images(embed: &Value) -> Vec<ImageRef> {
    embed
        .get("images")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|image| {
            let thumb_url = string_field(image, "thumb")?;
            Some(ImageRef {
                thumb_url,
                fullsize_url: string_field(image, "fullsize"),
                alt: string_field(image, "alt").filter(|alt| !alt.is_empty()),
            })
        })
        .collect()
}

fn parse_video(embed: &Value) -> Option<VideoRef> {
    Some(VideoRef {
        playlist_url: string_field(embed, "playlist")?,
        thumb_url: string_field(embed, "thumbnail"),
        alt: string_field(embed, "alt").filter(|alt| !alt.is_empty()),
        cid: string_field(embed, "cid"),
        aspect_ratio: embed.get("aspectRatio").and_then(|ratio| {
            Some((
                number_field_opt(ratio, "width")?,
                number_field_opt(ratio, "height")?,
            ))
        }),
    })
}

fn parse_external(embed: &Value) -> Option<ExternalRef> {
    let external = embed.get("external")?;
    Some(ExternalRef {
        uri: string_field(external, "uri").unwrap_or_default(),
        title: string_field(external, "title").unwrap_or_else(|| "external link".into()),
        description: string_field(external, "description").filter(|s| !s.is_empty()),
        thumb_url: string_field(external, "thumb"),
    })
}

fn post_text(post: &Value) -> String {
    record_text_field(post, "text").unwrap_or_default()
}

fn post_links(post: &Value, source: LinkSource) -> Vec<LinkRef> {
    let Some(record) = post.get("record") else {
        return Vec::new();
    };
    let Some(text) = string_field(record, "text") else {
        return Vec::new();
    };
    text_links(&text, record.get("facets"), source)
}

fn viewer_record_uri(post: &Value, field: &str) -> Option<String> {
    post.get("viewer")
        .and_then(|viewer| string_field(viewer, field))
}

fn post_reply_root(post: &Value) -> Option<PostRef> {
    let root = post
        .get("record")
        .and_then(|record| record.get("reply"))
        .and_then(|reply| reply.get("root"))?;
    Some(PostRef {
        uri: string_field(root, "uri")?,
        cid: string_field(root, "cid")?,
    })
}

fn quote_record_links(record: &Value) -> Vec<LinkRef> {
    let text = record_value_text(record);
    if text.is_empty() {
        return Vec::new();
    }
    let facets = record.get("value").and_then(|value| value.get("facets"));
    text_links(&text, facets, LinkSource::Text)
}

fn text_links(text: &str, facets: Option<&Value>, source: LinkSource) -> Vec<LinkRef> {
    let mut candidates = facet_links(text, facets, source);
    candidates.extend(plain_text_links(text, source));
    candidates.sort_by_key(|candidate| candidate.start);
    dedupe_links(
        candidates
            .into_iter()
            .map(|candidate| candidate.link)
            .collect(),
    )
}

#[derive(Debug, Clone)]
struct LinkCandidate {
    start: usize,
    link: LinkRef,
}

fn facet_links(text: &str, facets: Option<&Value>, source: LinkSource) -> Vec<LinkCandidate> {
    facets
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|facet| {
            let index = facet.get("index")?;
            let start = index.get("byteStart")?.as_u64()? as usize;
            let end = index.get("byteEnd")?.as_u64()? as usize;
            let label = text.get(start..end)?.to_owned();
            let features = facet.get("features")?.as_array()?;
            let uri = features.iter().find_map(|feature| {
                let feature_type = string_field(feature, "$type").unwrap_or_default();
                if feature_type == "app.bsky.richtext.facet#link" {
                    string_field(feature, "uri")
                } else {
                    None
                }
            })?;
            Some(LinkCandidate {
                start,
                link: LinkRef { uri, label, source },
            })
        })
        .collect()
}

fn plain_text_links(text: &str, source: LinkSource) -> Vec<LinkCandidate> {
    let mut finder = LinkFinder::new();
    finder.kinds(&[LinkKind::Url]);
    finder
        .links(text)
        .map(|link| LinkCandidate {
            start: link.start(),
            link: LinkRef {
                uri: link.as_str().to_owned(),
                label: link.as_str().to_owned(),
                source,
            },
        })
        .collect()
}

fn dedupe_links(links: Vec<LinkRef>) -> Vec<LinkRef> {
    let mut seen = std::collections::HashSet::new();
    links
        .into_iter()
        .filter(|link| !link.uri.is_empty())
        .filter(|link| seen.insert(link.uri.clone()))
        .collect()
}

fn record_text_field(post: &Value, field: &str) -> Option<String> {
    post.get("record")
        .and_then(|record| string_field(record, field))
}

fn record_value_text(record: &Value) -> String {
    record_value_field(record, "text").unwrap_or_default()
}

fn record_value_field(record: &Value, field: &str) -> Option<String> {
    record
        .get("value")
        .and_then(|value| string_field(value, field))
        .or_else(|| {
            record
                .get("record")
                .and_then(|value| string_field(value, field))
        })
}

fn display_name(author: &Value) -> String {
    string_field(author, "displayName")
        .filter(|name| !name.trim().is_empty())
        .or_else(|| string_field(author, "handle"))
        .unwrap_or_else(|| "unknown".into())
}

fn author_following(author: &Value) -> Option<bool> {
    author
        .get("viewer")
        .and_then(|viewer| viewer.get("following"))
        .map(|value| {
            value
                .as_str()
                .is_some_and(|following| !following.is_empty())
        })
}

fn string_field(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
}

fn bool_field(value: &Value, field: &str) -> Option<bool> {
    value.get(field).and_then(Value::as_bool)
}

fn number_field(value: &Value, field: &str) -> u64 {
    number_field_opt(value, field).unwrap_or_default()
}

fn number_field_opt(value: &Value, field: &str) -> Option<u64> {
    value.get(field).and_then(Value::as_u64)
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_owned()
}

pub fn compact_time(value: Option<&str>) -> String {
    let Some(value) = value else {
        return String::new();
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(value) else {
        return value.to_owned();
    };

    let now = Utc::now();
    let then = parsed.with_timezone(&Utc);
    let delta = now.signed_duration_since(then);

    if delta.num_seconds() < 60 {
        "now".into()
    } else if delta.num_minutes() < 60 {
        format!("{}m", delta.num_minutes())
    } else if delta.num_hours() < 24 {
        format!("{}h", delta.num_hours())
    } else if delta.num_days() < 14 {
        format!("{}d", delta.num_days())
    } else {
        then.format("%Y-%m-%d").to_string()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_plain_timeline_post() {
        let root = json!({
            "cursor": "next",
            "feed": [{
                "post": {
                    "uri": "at://did:plc:alice/app.bsky.feed.post/1",
                    "cid": "cid1",
                    "viewer": {
                        "like": "at://did:plc:viewer/app.bsky.feed.like/1",
                        "repost": "at://did:plc:viewer/app.bsky.feed.repost/1"
                    },
                    "author": {"handle": "alice.test", "displayName": "Alice"},
                    "record": {
                        "text": "hello terminal",
                        "createdAt": "2026-05-22T00:00:00Z",
                        "reply": {
                            "root": {"uri": "at://did:plc:root/app.bsky.feed.post/1", "cid": "rootcid"}
                        }
                    },
                    "replyCount": 2,
                    "repostCount": 3,
                    "likeCount": 5,
                    "quoteCount": 1,
                    "indexedAt": "2026-05-22T00:01:00Z"
                }
            }]
        });

        let (items, cursor) = timeline_items(&root, &HomeFeedPrefs::default());
        assert_eq!(cursor.as_deref(), Some("next"));
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author_name, "Alice");
        assert_eq!(items[0].text, "hello terminal");
        assert_eq!(items[0].reply_count, 2);
        assert_eq!(
            items[0].viewer_like.as_deref(),
            Some("at://did:plc:viewer/app.bsky.feed.like/1")
        );
        assert_eq!(
            items[0].viewer_repost.as_deref(),
            Some("at://did:plc:viewer/app.bsky.feed.repost/1")
        );
        assert_eq!(
            items[0].reply_root,
            Some(PostRef {
                uri: "at://did:plc:root/app.bsky.feed.post/1".into(),
                cid: "rootcid".into()
            })
        );
    }

    #[test]
    fn parses_repost_reason() {
        let root = json!({
            "feed": [{
                "post": {
                    "uri": "at://did:plc:bob/app.bsky.feed.post/1",
                    "author": {"handle": "bob.test"},
                    "record": {"text": "reposted text"}
                },
                "reason": {
                    "$type": "app.bsky.feed.defs#reasonRepost",
                    "by": {"handle": "alice.test", "displayName": "Alice"},
                    "indexedAt": "2026-05-22T00:02:00Z"
                }
            }]
        });

        let (items, _) = timeline_items(&root, &HomeFeedPrefs::default());
        assert_eq!(
            items[0].reason,
            Some(FeedReason::Repost {
                by_name: "Alice".into(),
                by_handle: "alice.test".into(),
                indexed_at: Some("2026-05-22T00:02:00Z".into())
            })
        );
    }

    #[test]
    fn parses_pinned_reason() {
        let entry = json!({
            "post": {
                "uri": "at://did:plc:alice/app.bsky.feed.post/1",
                "author": {"handle": "alice.test"},
                "record": {"text": "pinned text"}
            },
            "reason": {"$type": "app.bsky.feed.defs#reasonPin"}
        });

        let item = feed_item_from_feed_entry(&entry).unwrap();
        assert_eq!(item.reason, Some(FeedReason::Pin));
    }

    #[test]
    fn parses_reply_context_with_parent_preview() {
        let root = json!({
            "feed": [{
                "post": {
                    "uri": "at://did:plc:carol/app.bsky.feed.post/reply",
                    "author": {"handle": "carol.test", "viewer": {"following": "at://did:viewer/follow/1"}},
                    "record": {"text": "reply text"},
                    "likeCount": 4
                },
                "reply": {
                    "root": {
                        "uri": "at://did:plc:alice/app.bsky.feed.post/root",
                        "author": {"handle": "alice.test"},
                        "record": {"text": "root text"}
                    },
                    "parent": {
                        "uri": "at://did:plc:bob/app.bsky.feed.post/parent",
                        "author": {"handle": "bob.test", "displayName": "Bob"},
                        "record": {"text": "parent text"}
                    },
                    "grandparentAuthor": {"handle": "alice.test"}
                }
            }]
        });

        let (items, _) = timeline_items(&root, &HomeFeedPrefs::default());
        let reply = items[0].reply.as_ref().unwrap();
        assert_eq!(reply.root_uri, "at://did:plc:alice/app.bsky.feed.post/root");
        assert_eq!(reply.parent_author_name, "Bob");
        assert_eq!(reply.parent_author_handle, "bob.test");
        assert_eq!(reply.parent_text, "parent text");
        assert_eq!(
            reply.grandparent_author_handle.as_deref(),
            Some("alice.test")
        );
        assert_eq!(items[0].author_following, Some(true));
    }

    #[test]
    fn parses_reply_context_with_blocked_parent() {
        let entry = json!({
            "post": {
                "uri": "at://did:plc:carol/app.bsky.feed.post/reply",
                "author": {"handle": "carol.test"},
                "record": {"text": "reply text"}
            },
            "reply": {
                "root": {"uri": "at://did:plc:alice/app.bsky.feed.post/root", "notFound": true},
                "parent": {
                    "uri": "at://did:plc:bob/app.bsky.feed.post/parent",
                    "blocked": true,
                    "author": {"did": "did:plc:bob"}
                }
            }
        });

        let item = feed_item_from_feed_entry(&entry).unwrap();
        let reply = item.reply.unwrap();
        assert_eq!(reply.parent_author_handle, "blocked");
        assert_eq!(reply.parent_text, "[blocked post]");
        assert_eq!(reply.parent_status, Some(ReplyParentStatus::Blocked));
    }

    #[test]
    fn parses_home_feed_preferences() {
        let root = json!({
            "preferences": [
                {"$type": "app.bsky.actor.defs#feedViewPref", "feed": "other", "hideReplies": true},
                {
                    "$type": "app.bsky.actor.defs#feedViewPref",
                    "feed": "home",
                    "hideReplies": true,
                    "hideRepliesByUnfollowed": true,
                    "hideRepliesByLikeCount": 3,
                    "hideReposts": true,
                    "hideQuotePosts": true
                }
            ]
        });

        let prefs = HomeFeedPrefs::from_preferences_response(&root);
        assert!(prefs.hide_replies);
        assert!(prefs.hide_replies_by_unfollowed);
        assert_eq!(prefs.hide_replies_by_like_count, Some(3));
        assert!(prefs.hide_reposts);
        assert!(prefs.hide_quote_posts);
    }

    #[test]
    fn parses_saved_feeds_pref_v2_and_ignores_lists() {
        let root = json!({
            "preferences": [{
                "$type": "app.bsky.actor.defs#savedFeedsPrefV2",
                "items": [
                    {
                        "type": "feed",
                        "value": "at://did:plc:alice/app.bsky.feed.generator/for-you",
                        "pinned": true
                    },
                    {
                        "type": "list",
                        "value": "at://did:plc:alice/app.bsky.graph.list/news"
                    },
                    {
                        "type": "feed",
                        "value": "at://did:plc:alice/app.bsky.feed.generator/for-you"
                    }
                ]
            }]
        });

        let sources = feed_sources_from_preferences(&root);

        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0], FeedSource::home());
        assert_eq!(sources[1].label, "for-you");
        assert_eq!(
            sources[1].uri(),
            Some("at://did:plc:alice/app.bsky.feed.generator/for-you")
        );
    }

    #[test]
    fn adds_author_feed_for_active_account() {
        let root = json!({
            "preferences": [{
                "$type": "app.bsky.actor.defs#savedFeedsPrefV2",
                "items": [{
                    "type": "feed",
                    "value": "at://did:plc:alice/app.bsky.feed.generator/for-you"
                }]
            }]
        });

        let sources = feed_sources_for_account(&root, "alice.test", "did:plc:alice");

        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0], FeedSource::home());
        assert_eq!(
            sources[1],
            FeedSource::author("alice.test", "did:plc:alice")
        );
        assert_eq!(sources[2].label, "for-you");
    }

    #[test]
    fn parses_legacy_saved_feed_preferences() {
        let root = json!({
            "preferences": [{
                "$type": "app.bsky.actor.defs#savedFeedsPref",
                "pinned": [
                    "at://did:plc:alice/app.bsky.feed.generator/news",
                    "at://did:plc:alice/app.bsky.graph.list/list"
                ],
                "saved": [
                    "at://did:plc:bob/app.bsky.feed.generator/science"
                ]
            }]
        });

        let sources = feed_sources_from_preferences(&root);
        let labels = sources
            .iter()
            .map(|source| source.label.as_str())
            .collect::<Vec<_>>();

        assert_eq!(labels, vec!["Following", "news", "science"]);
    }

    #[test]
    fn filters_timeline_with_home_preferences() {
        let root = json!({
            "feed": [
                {
                    "post": {"uri": "repost", "author": {"handle": "bob.test"}, "record": {"text": "repost"}},
                    "reason": {"$type": "app.bsky.feed.defs#reasonRepost", "by": {"handle": "alice.test"}, "indexedAt": "2026-05-22T00:00:00Z"}
                },
                {
                    "post": {"uri": "reply", "author": {"handle": "bob.test"}, "record": {"text": "reply"}, "likeCount": 0},
                    "reply": {
                        "root": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}},
                        "parent": {"uri": "parent", "author": {"handle": "alice.test"}, "record": {"text": "parent"}}
                    }
                },
                {
                    "post": {
                        "uri": "quote",
                        "author": {"handle": "bob.test"},
                        "record": {"text": "quote"},
                        "embed": {
                            "$type": "app.bsky.embed.record#view",
                            "record": {
                                "$type": "app.bsky.embed.record#viewRecord",
                                "uri": "quoted",
                                "author": {"handle": "alice.test"},
                                "value": {"text": "quoted"}
                            }
                        }
                    }
                },
                {
                    "post": {"uri": "normal", "author": {"handle": "bob.test"}, "record": {"text": "normal"}}
                }
            ]
        });
        let prefs = HomeFeedPrefs {
            hide_replies: true,
            hide_reposts: true,
            hide_quote_posts: true,
            ..HomeFeedPrefs::default()
        };

        let (items, _) = timeline_items(&root, &prefs);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uri, "normal");
    }

    #[test]
    fn filters_replies_by_like_count_and_follow_state() {
        let root = json!({
            "feed": [
                {
                    "post": {
                        "uri": "unfollowed",
                        "author": {"handle": "bob.test", "viewer": {"following": null}},
                        "record": {"text": "reply"},
                        "likeCount": 10
                    },
                    "reply": {
                        "root": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}},
                        "parent": {"uri": "parent", "author": {"handle": "alice.test"}, "record": {"text": "parent"}}
                    }
                },
                {
                    "post": {
                        "uri": "low-like",
                        "author": {"handle": "carol.test", "viewer": {"following": "at://follow"}},
                        "record": {"text": "reply"},
                        "likeCount": 1
                    },
                    "reply": {
                        "root": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}},
                        "parent": {"uri": "parent", "author": {"handle": "alice.test"}, "record": {"text": "parent"}}
                    }
                },
                {
                    "post": {
                        "uri": "kept",
                        "author": {"handle": "dana.test", "viewer": {"following": "at://follow"}},
                        "record": {"text": "reply"},
                        "likeCount": 5
                    },
                    "reply": {
                        "root": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}},
                        "parent": {"uri": "parent", "author": {"handle": "alice.test"}, "record": {"text": "parent"}}
                    }
                }
            ]
        });
        let prefs = HomeFeedPrefs {
            hide_replies_by_unfollowed: true,
            hide_replies_by_like_count: Some(3),
            ..HomeFeedPrefs::default()
        };

        let (items, _) = timeline_items(&root, &prefs);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uri, "kept");
    }

    #[test]
    fn parses_quote_with_media() {
        let post = json!({
            "uri": "at://did:plc:bob/app.bsky.feed.post/2",
            "author": {"handle": "bob.test"},
            "record": {"text": "look at this"},
            "embed": {
                "$type": "app.bsky.embed.recordWithMedia#view",
                "media": {
                    "$type": "app.bsky.embed.images#view",
                    "images": [{"thumb": "https://example.com/thumb.jpg", "fullsize": "https://example.com/full.jpg", "alt": "alt text"}]
                },
                "record": {
                    "$type": "app.bsky.embed.record#viewRecord",
                    "uri": "at://did:plc:alice/app.bsky.feed.post/1",
                    "author": {"handle": "alice.test", "displayName": "Alice"},
                    "value": {"text": "quoted text", "createdAt": "2026-05-22T00:00:00Z"}
                }
            }
        });

        let item = feed_item_from_post(&post, 0);
        assert_eq!(item.images.len(), 1);
        let quote = item.quote.unwrap();
        assert_eq!(quote.author_name, "Alice");
        assert_eq!(quote.text, "quoted text");
    }

    #[test]
    fn parses_video_embed() {
        let post = json!({
            "uri": "at://did:plc:bob/app.bsky.feed.post/2",
            "author": {"handle": "bob.test"},
            "record": {"text": "watch"},
            "embed": {
                "$type": "app.bsky.embed.video#view",
                "playlist": "https://video.bsky.app/watch/playlist.m3u8",
                "thumbnail": "https://video.bsky.app/watch/thumb.jpg",
                "alt": "video alt",
                "cid": "videocid",
                "aspectRatio": {"width": 16, "height": 9}
            }
        });

        let item = feed_item_from_post(&post, 0);

        assert_eq!(item.videos.len(), 1);
        assert_eq!(
            item.videos[0],
            VideoRef {
                playlist_url: "https://video.bsky.app/watch/playlist.m3u8".into(),
                thumb_url: Some("https://video.bsky.app/watch/thumb.jpg".into()),
                alt: Some("video alt".into()),
                cid: Some("videocid".into()),
                aspect_ratio: Some((16, 9))
            }
        );
    }

    #[test]
    fn collects_external_facet_plain_and_quote_links_in_order() {
        let post = json!({
            "uri": "at://did:plc:bob/app.bsky.feed.post/2",
            "author": {"handle": "bob.test"},
            "record": {
                "text": "go docs and https://plain.test",
                "facets": [{
                    "index": {"byteStart": 3, "byteEnd": 7},
                    "features": [{
                        "$type": "app.bsky.richtext.facet#link",
                        "uri": "https://docs.test"
                    }]
                }]
            },
            "embed": {
                "$type": "app.bsky.embed.recordWithMedia#view",
                "media": {
                    "$type": "app.bsky.embed.external#view",
                    "external": {
                        "uri": "https://card.test",
                        "title": "Card"
                    }
                },
                "record": {
                    "$type": "app.bsky.embed.record#viewRecord",
                    "uri": "at://did:plc:alice/app.bsky.feed.post/1",
                    "author": {"handle": "alice.test", "displayName": "Alice"},
                    "value": {
                        "text": "quote https://quote-text.test"
                    },
                    "embed": {
                        "$type": "app.bsky.embed.external#view",
                        "external": {
                            "uri": "https://quote-card.test",
                            "title": "Quote Card"
                        }
                    }
                }
            }
        });

        let item = feed_item_from_post(&post, 0);
        let links = item_links(&item);
        let uris = links
            .iter()
            .map(|link| link.uri.as_str())
            .collect::<Vec<_>>();
        let sources = links.iter().map(|link| link.source).collect::<Vec<_>>();

        assert_eq!(
            uris,
            vec![
                "https://card.test",
                "https://docs.test",
                "https://plain.test",
                "https://quote-card.test",
                "https://quote-text.test"
            ]
        );
        assert_eq!(
            sources,
            vec![
                LinkSource::External,
                LinkSource::Text,
                LinkSource::Text,
                LinkSource::QuoteExternal,
                LinkSource::QuoteText
            ]
        );
    }

    #[test]
    fn skips_invalid_facet_byte_ranges() {
        let post = json!({
            "uri": "at://did:plc:bob/app.bsky.feed.post/2",
            "author": {"handle": "bob.test"},
            "record": {
                "text": "héllo",
                "facets": [{
                "index": {"byteStart": 2, "byteEnd": 3},
                    "features": [{
                        "$type": "app.bsky.richtext.facet#link",
                        "uri": "https://invalid.test"
                    }]
                }]
            }
        });

        let item = feed_item_from_post(&post, 0);

        assert!(item_links(&item).is_empty());
    }

    #[test]
    fn handles_blocked_quote() {
        let post = json!({
            "uri": "at://did:plc:bob/app.bsky.feed.post/2",
            "author": {"handle": "bob.test"},
            "record": {"text": "blocked quote"},
            "embed": {
                "$type": "app.bsky.embed.record#view",
                "record": {"$type": "app.bsky.embed.record#viewBlocked"}
            }
        });

        let item = feed_item_from_post(&post, 0);
        assert!(item.quote.is_none());
        assert_eq!(item.embed_status.as_deref(), Some("[quoted post blocked]"));
    }

    #[test]
    fn flattens_thread_replies() {
        let root = json!({
            "thread": {
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}},
                "replies": [{
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {"uri": "reply", "author": {"handle": "bob.test"}, "record": {"text": "reply"}},
                    "replies": []
                }]
            }
        });

        let items = thread_items(&root);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].depth, 0);
        assert_eq!(items[1].depth, 1);
    }

    #[test]
    fn flattens_thread_parent_chain_before_selected_post() {
        let root = json!({
            "thread": {
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "selected", "author": {"handle": "carol.test"}, "record": {"text": "selected"}},
                "parent": {
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {"uri": "parent", "author": {"handle": "bob.test"}, "record": {"text": "parent"}},
                    "parent": {
                        "$type": "app.bsky.feed.defs#threadViewPost",
                        "post": {"uri": "root", "author": {"handle": "alice.test"}, "record": {"text": "root"}}
                    }
                },
                "replies": [{
                    "$type": "app.bsky.feed.defs#threadViewPost",
                    "post": {"uri": "reply", "author": {"handle": "dana.test"}, "record": {"text": "reply"}}
                }]
            }
        });

        let items = thread_items(&root);
        let uris = items
            .iter()
            .map(|item| (item.uri.as_str(), item.depth))
            .collect::<Vec<_>>();
        assert_eq!(
            uris,
            vec![("root", 0), ("parent", 1), ("selected", 2), ("reply", 3)]
        );
    }

    #[test]
    fn skips_blocked_and_missing_thread_ancestors() {
        let root = json!({
            "thread": {
                "$type": "app.bsky.feed.defs#threadViewPost",
                "post": {"uri": "selected", "author": {"handle": "carol.test"}, "record": {"text": "selected"}},
                "parent": {
                    "$type": "app.bsky.feed.defs#blockedPost",
                    "uri": "blocked-parent"
                },
                "replies": [{
                    "$type": "app.bsky.feed.defs#notFoundPost",
                    "uri": "missing-reply"
                }]
            }
        });

        let items = thread_items(&root);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].uri, "selected");
        assert_eq!(items[0].depth, 0);
    }
}
