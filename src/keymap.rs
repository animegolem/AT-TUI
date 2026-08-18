use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputContext {
    Global,
    Normal,
    Search,
    Menu,
    Media,
    Links,
    Composer,
    Confirmation,
}

impl InputContext {
    fn readme_label(self) -> &'static str {
        match self {
            Self::Global => "Global",
            Self::Normal => "Normal",
            Self::Search => "Search",
            Self::Menu => "Menu",
            Self::Media => "Media overlay",
            Self::Links => "Link picker",
            Self::Composer => "Composer",
            Self::Confirmation => "Delete confirmation",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    BackOrQuit,
    MoveDown,
    MoveUp,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    PageUp,
    OpenMenu,
    PreviewMedia,
    OpenLinks,
    PreviousFeed,
    NextFeed,
    LoadPending,
    ToggleLike,
    ToggleRepost,
    ToggleFollow,
    DeleteOwnPost,
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
    CloseOverlay,
    MenuNextSection,
    MenuPreviousSection,
    MenuTabNext,
    MenuTabPrevious,
    MediaPrevious,
    MediaNext,
    PlayVideo,
    OpenMediaExternally,
    LinkNext,
    LinkPrevious,
    OpenLink,
    ConfirmDelete,
    CancelDelete,
    SubmitComposer,
    InsertNewline,
    Backspace,
    SubmitSearch,
    CancelSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ModifierPattern {
    Any,
    Contains(KeyModifiers),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct KeyPattern {
    code: KeyCode,
    modifiers: ModifierPattern,
}

impl KeyPattern {
    const fn any(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: ModifierPattern::Any,
        }
    }

    const fn with(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self {
            code,
            modifiers: ModifierPattern::Contains(modifiers),
        }
    }

    fn matches(self, key: KeyEvent) -> bool {
        self.code == key.code
            && match self.modifiers {
                ModifierPattern::Any => true,
                ModifierPattern::Contains(modifiers) => key.modifiers.contains(modifiers),
            }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HelpGroup {
    Movement,
    Navigation,
    Surfaces,
    Actions,
    Writing,
    Feeds,
    Controls,
}

#[derive(Debug, Clone, Copy)]
struct Help {
    group: HelpGroup,
    keys: &'static str,
    description: &'static str,
    compact: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
struct Binding {
    context: InputContext,
    pattern: KeyPattern,
    action: KeyAction,
    help: Option<Help>,
}

impl Binding {
    const fn visible(
        context: InputContext,
        pattern: KeyPattern,
        action: KeyAction,
        group: HelpGroup,
        keys: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            context,
            pattern,
            action,
            help: Some(Help {
                group,
                keys,
                description,
                compact: None,
            }),
        }
    }

    const fn compact(
        context: InputContext,
        pattern: KeyPattern,
        action: KeyAction,
        group: HelpGroup,
        keys: &'static str,
        description: &'static str,
        compact: &'static str,
    ) -> Self {
        Self {
            context,
            pattern,
            action,
            help: Some(Help {
                group,
                keys,
                description,
                compact: Some(compact),
            }),
        }
    }

    const fn alias(context: InputContext, pattern: KeyPattern, action: KeyAction) -> Self {
        Self {
            context,
            pattern,
            action,
            help: None,
        }
    }
}

use HelpGroup as Group;
use InputContext as Context;
use KeyAction as Action;

const BINDINGS: &[Binding] = &[
    Binding::compact(
        Context::Global,
        KeyPattern::with(KeyCode::Char('c'), KeyModifiers::CONTROL),
        Action::Quit,
        Group::Controls,
        "Ctrl-C",
        "quit",
        "Ctrl-C quit · ? menu",
    ),
    Binding::compact(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('j')),
        Action::MoveDown,
        Group::Movement,
        "j/k or arrows",
        "move",
        "j/k/arrows move · Ctrl-d/u half page · PgUp/PgDn page",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Down),
        Action::MoveDown,
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('k')),
        Action::MoveUp,
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Up),
        Action::MoveUp,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::with(KeyCode::Char('d'), KeyModifiers::CONTROL),
        Action::HalfPageDown,
        Group::Movement,
        "Ctrl-d/Ctrl-u",
        "half page",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::with(KeyCode::Char('u'), KeyModifiers::CONTROL),
        Action::HalfPageUp,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::PageUp),
        Action::PageUp,
        Group::Movement,
        "PgUp/PgDn",
        "page",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::PageDown),
        Action::PageDown,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('g')),
        Action::JumpTop,
        Group::Movement,
        "g/G",
        "top/bottom",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('G')),
        Action::JumpBottom,
    ),
    Binding::compact(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('l')),
        Action::OpenSelected,
        Group::Navigation,
        "l/Enter/Right",
        "open selected",
        "l/Enter/Right open · h/Left/Esc back · q back/quit",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Enter),
        Action::OpenSelected,
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Right),
        Action::OpenSelected,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('h')),
        Action::Back,
        Group::Navigation,
        "h/Left",
        "back",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Left),
        Action::Back,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Esc),
        Action::Escape,
        Group::Navigation,
        "Esc",
        "back/settings",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('q')),
        Action::BackOrQuit,
        Group::Navigation,
        "q",
        "back; quit at root",
    ),
    Binding::compact(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('P')),
        Action::OpenProfile,
        Group::Surfaces,
        "P",
        "profile",
        "P profile · N notifications · Space media · o links · e quote",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('N')),
        Action::OpenNotifications,
        Group::Surfaces,
        "N",
        "notifications",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char(' ')),
        Action::PreviewMedia,
        Group::Navigation,
        "Space",
        "media",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('o')),
        Action::OpenLinks,
        Group::Navigation,
        "o",
        "links",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('e')),
        Action::OpenQuote,
        Group::Navigation,
        "e",
        "quoted post",
    ),
    Binding::compact(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('f')),
        Action::ToggleLike,
        Group::Actions,
        "f",
        "like",
        "f/b/F like/repost/follow · p/r/Q post/reply/quote · d delete",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('b')),
        Action::ToggleRepost,
        Group::Actions,
        "b",
        "repost",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('F')),
        Action::ToggleFollow,
        Group::Actions,
        "F",
        "follow",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('d')),
        Action::DeleteOwnPost,
        Group::Actions,
        "d",
        "delete",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('p')),
        Action::ComposePost,
        Group::Writing,
        "p",
        "post",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('r')),
        Action::ComposeReply,
        Group::Writing,
        "r",
        "reply",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('Q')),
        Action::ComposeQuote,
        Group::Writing,
        "Q",
        "quote",
    ),
    Binding::compact(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('[')),
        Action::PreviousFeed,
        Group::Feeds,
        "[/]",
        "previous/next feed",
        "[/] feeds · / search · n next · R reload · u pending",
    ),
    Binding::alias(
        Context::Normal,
        KeyPattern::any(KeyCode::Char(']')),
        Action::NextFeed,
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('/')),
        Action::StartSearch,
        Group::Feeds,
        "/",
        "search",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('n')),
        Action::SearchNext,
        Group::Feeds,
        "n",
        "next match",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('R')),
        Action::Reload,
        Group::Feeds,
        "R",
        "reload",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('u')),
        Action::LoadPending,
        Group::Feeds,
        "u",
        "load pending",
    ),
    Binding::visible(
        Context::Normal,
        KeyPattern::any(KeyCode::Char('?')),
        Action::OpenMenu,
        Group::Controls,
        "?",
        "menu",
    ),
    Binding::visible(
        Context::Search,
        KeyPattern::any(KeyCode::Esc),
        Action::CancelSearch,
        Group::Controls,
        "Esc",
        "cancel",
    ),
    Binding::visible(
        Context::Search,
        KeyPattern::any(KeyCode::Enter),
        Action::SubmitSearch,
        Group::Controls,
        "Enter",
        "search",
    ),
    Binding::visible(
        Context::Search,
        KeyPattern::any(KeyCode::Backspace),
        Action::Backspace,
        Group::Controls,
        "Backspace",
        "delete character",
    ),
    Binding::compact(
        Context::Menu,
        KeyPattern::any(KeyCode::Esc),
        Action::CloseOverlay,
        Group::Controls,
        "Esc/?/Enter/q",
        "close",
        "j/k sections · Tab action · [/] feeds · Esc/?/Enter/q close",
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Char('?')),
        Action::CloseOverlay,
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Enter),
        Action::CloseOverlay,
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Char('q')),
        Action::CloseOverlay,
    ),
    Binding::visible(
        Context::Menu,
        KeyPattern::any(KeyCode::Char('j')),
        Action::MenuNextSection,
        Group::Movement,
        "j/k or arrows",
        "change section",
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Down),
        Action::MenuNextSection,
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Char('k')),
        Action::MenuPreviousSection,
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Up),
        Action::MenuPreviousSection,
    ),
    Binding::visible(
        Context::Menu,
        KeyPattern::any(KeyCode::Tab),
        Action::MenuTabNext,
        Group::Navigation,
        "Tab/Shift-Tab",
        "section action",
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::BackTab),
        Action::MenuTabPrevious,
    ),
    Binding::visible(
        Context::Menu,
        KeyPattern::any(KeyCode::Char('[')),
        Action::PreviousFeed,
        Group::Feeds,
        "[/]",
        "previous/next feed",
    ),
    Binding::alias(
        Context::Menu,
        KeyPattern::any(KeyCode::Char(']')),
        Action::NextFeed,
    ),
    Binding::compact(
        Context::Media,
        KeyPattern::any(KeyCode::Esc),
        Action::CloseOverlay,
        Group::Controls,
        "Space/Esc/q",
        "close",
        "h/l switch · Enter/p play · u open · Space/Esc/q close",
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Char(' ')),
        Action::CloseOverlay,
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Char('q')),
        Action::CloseOverlay,
    ),
    Binding::visible(
        Context::Media,
        KeyPattern::any(KeyCode::Char('h')),
        Action::MediaPrevious,
        Group::Movement,
        "h/l or arrows",
        "switch",
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Left),
        Action::MediaPrevious,
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Char('l')),
        Action::MediaNext,
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Right),
        Action::MediaNext,
    ),
    Binding::visible(
        Context::Media,
        KeyPattern::any(KeyCode::Enter),
        Action::PlayVideo,
        Group::Actions,
        "Enter/p",
        "play video",
    ),
    Binding::alias(
        Context::Media,
        KeyPattern::any(KeyCode::Char('p')),
        Action::PlayVideo,
    ),
    Binding::visible(
        Context::Media,
        KeyPattern::any(KeyCode::Char('u')),
        Action::OpenMediaExternally,
        Group::Actions,
        "u",
        "open externally",
    ),
    Binding::compact(
        Context::Links,
        KeyPattern::any(KeyCode::Esc),
        Action::CloseOverlay,
        Group::Controls,
        "Esc/q",
        "close",
        "j/k move · Enter/u open · Esc/q close",
    ),
    Binding::alias(
        Context::Links,
        KeyPattern::any(KeyCode::Char('q')),
        Action::CloseOverlay,
    ),
    Binding::visible(
        Context::Links,
        KeyPattern::any(KeyCode::Char('j')),
        Action::LinkNext,
        Group::Movement,
        "j/k or arrows",
        "move",
    ),
    Binding::alias(
        Context::Links,
        KeyPattern::any(KeyCode::Down),
        Action::LinkNext,
    ),
    Binding::alias(
        Context::Links,
        KeyPattern::any(KeyCode::Char('k')),
        Action::LinkPrevious,
    ),
    Binding::alias(
        Context::Links,
        KeyPattern::any(KeyCode::Up),
        Action::LinkPrevious,
    ),
    Binding::visible(
        Context::Links,
        KeyPattern::any(KeyCode::Enter),
        Action::OpenLink,
        Group::Actions,
        "Enter/u",
        "open",
    ),
    Binding::alias(
        Context::Links,
        KeyPattern::any(KeyCode::Char('u')),
        Action::OpenLink,
    ),
    Binding::compact(
        Context::Composer,
        KeyPattern::any(KeyCode::Esc),
        Action::CloseOverlay,
        Group::Controls,
        "Esc",
        "cancel",
        "Text type · Enter newline · Ctrl-S send · Esc cancel",
    ),
    Binding::visible(
        Context::Composer,
        KeyPattern::with(KeyCode::Char('s'), KeyModifiers::CONTROL),
        Action::SubmitComposer,
        Group::Actions,
        "Ctrl-S",
        "send",
    ),
    Binding::visible(
        Context::Composer,
        KeyPattern::any(KeyCode::Enter),
        Action::InsertNewline,
        Group::Writing,
        "Enter",
        "newline",
    ),
    Binding::visible(
        Context::Composer,
        KeyPattern::any(KeyCode::Backspace),
        Action::Backspace,
        Group::Writing,
        "Backspace",
        "delete character",
    ),
    Binding::compact(
        Context::Confirmation,
        KeyPattern::any(KeyCode::Char('y')),
        Action::ConfirmDelete,
        Group::Actions,
        "y/Y",
        "delete",
        "y/Y delete · any other key cancel",
    ),
    Binding::alias(
        Context::Confirmation,
        KeyPattern::any(KeyCode::Char('Y')),
        Action::ConfirmDelete,
    ),
];

const HELP_ONLY: &[(InputContext, Help)] = &[
    (
        Context::Composer,
        Help {
            group: Group::Writing,
            keys: "Text",
            description: "type normally",
            compact: None,
        },
    ),
    (
        Context::Confirmation,
        Help {
            group: Group::Controls,
            keys: "Any other key",
            description: "cancel",
            compact: None,
        },
    ),
    (
        Context::Search,
        Help {
            group: Group::Writing,
            keys: "Text",
            description: "type query",
            compact: None,
        },
    ),
];

pub fn action_for_key(context: InputContext, key: KeyEvent) -> Option<KeyAction> {
    BINDINGS
        .iter()
        .find(|binding| {
            (binding.context == InputContext::Global || binding.context == context)
                && binding.pattern.matches(key)
        })
        .map(|binding| binding.action)
        .or_else(|| (context == InputContext::Confirmation).then_some(KeyAction::CancelDelete))
}

fn help_for_context(context: InputContext) -> Vec<Help> {
    let mut help = BINDINGS
        .iter()
        .filter(|binding| binding.context == context || binding.context == InputContext::Global)
        .filter_map(|binding| binding.help)
        .collect::<Vec<_>>();
    help.extend(
        HELP_ONLY
            .iter()
            .filter(|(help_context, _)| *help_context == context)
            .map(|(_, help)| *help),
    );
    help
}

pub fn help_lines(context: InputContext) -> Vec<String> {
    let mut lines = Vec::new();
    for group in [
        Group::Movement,
        Group::Navigation,
        Group::Surfaces,
        Group::Actions,
        Group::Writing,
        Group::Feeds,
        Group::Controls,
    ] {
        let entries = help_for_context(context)
            .into_iter()
            .filter(|help| help.group == group)
            .filter_map(|help| help.compact)
            .collect::<Vec<_>>();
        if !entries.is_empty() {
            lines.push(format!("  {}", entries.join(" · ")));
        }
    }
    lines
}

pub fn help_line(context: InputContext) -> String {
    help_for_context(context)
        .into_iter()
        .map(|help| format!("{} {}", help.keys, help.description))
        .collect::<Vec<_>>()
        .join(" · ")
}

pub fn compact_help_line(context: InputContext) -> String {
    BINDINGS
        .iter()
        .filter(|binding| binding.context == context)
        .filter_map(|binding| binding.help.and_then(|help| help.compact))
        .next()
        .map(str::to_owned)
        .unwrap_or_else(|| help_line(context))
}

pub fn readme_keymap() -> String {
    let mut output = String::new();
    for context in [
        Context::Normal,
        Context::Menu,
        Context::Media,
        Context::Composer,
        Context::Links,
        Context::Confirmation,
        Context::Search,
    ] {
        output.push_str(&format!("### {}\n\n", context.readme_label()));
        for help in help_for_context(context) {
            output.push_str(&format!("- `{}` — {}\n", help.keys, help.description));
        }
        output.push('\n');
    }
    output.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn bindings_do_not_conflict_within_a_context() {
        let mut seen = HashSet::new();
        for binding in BINDINGS {
            assert!(
                seen.insert((binding.context, binding.pattern)),
                "duplicate binding in {:?} for {:?}",
                binding.context,
                binding.pattern
            );
        }
    }

    #[test]
    fn contextual_space_and_enter_resolve_differently() {
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(
            action_for_key(Context::Normal, key(KeyCode::Char(' '))),
            Some(Action::PreviewMedia)
        );
        assert_eq!(
            action_for_key(Context::Media, key(KeyCode::Char(' '))),
            Some(Action::CloseOverlay)
        );
        assert_eq!(
            action_for_key(Context::Normal, key(KeyCode::Enter)),
            Some(Action::OpenSelected)
        );
        assert_eq!(
            action_for_key(Context::Composer, key(KeyCode::Enter)),
            Some(Action::InsertNewline)
        );
    }

    #[test]
    fn global_quit_and_retired_normal_keys_are_preserved() {
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let ctrl = |character| KeyEvent::new(KeyCode::Char(character), KeyModifiers::CONTROL);
        for context in [
            Context::Normal,
            Context::Menu,
            Context::Media,
            Context::Links,
            Context::Composer,
            Context::Confirmation,
            Context::Search,
        ] {
            assert_eq!(action_for_key(context, ctrl('c')), Some(Action::Quit));
        }
        for retired in ['w', 'U', 'c'] {
            assert_eq!(
                action_for_key(Context::Normal, key(KeyCode::Char(retired))),
                None
            );
        }
    }

    #[test]
    fn readme_keymap_is_generated_from_registry() {
        let readme = include_str!("../README.md");
        let start = readme
            .split_once("<!-- BEGIN GENERATED KEYMAP -->")
            .unwrap()
            .1;
        let documented = start
            .split_once("<!-- END GENERATED KEYMAP -->")
            .unwrap()
            .0
            .trim();
        assert_eq!(documented, readme_keymap());
    }
}
