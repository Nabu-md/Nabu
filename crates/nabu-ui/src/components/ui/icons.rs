//! # Nabu Icon System
//!
//! Centralized re-export wall and abstraction for every Lucide icon used in the
//! application.
//!
//! **Why a single module instead of ad-hoc imports everywhere?**
//!
//! - One import path for all icons: `use crate::components::ui::icons::*`.
//! - The [`Icon`] enum is the single source of truth for *which* icon appears
//!   wherever a UI element needs an icon (commands, tabs, sidebar items, empty
//!   states, toasts, …). Call sites never reference a Lucide component directly
//!   — they construct an [`Icon`] variant and hand it to [`render_icon`].
//! - Easy to audit: every mapping from a concept to a Lucide glyph lives in
//!   [`render_icon`].
//! - Easy to swap the icon library in the future — only this module and the
//!   [`Icon`] enum change.
//!
//! ## Theme support
//!
//! Lucide SVGs render with `stroke="currentColor"` (the Lucide default) so they
//! automatically inherit the surrounding text colour and adapt to Light / Dark /
//! System themes via the existing design tokens. No colours are hardcoded.
//!
//! ## Sizing
//!
//! The global `.lucide` rule (in `src/styles/app.css`) sets
//! `width: 1em; height: 1em` so every icon scales with the parent's
//! `font-size` — exactly like the emoji glyphs they replace. Call sites that
//! need a different footprint simply add a Tailwind `text-*` / `text-[...]`
//! class to the wrapping element, which is how every emoji was sized before.
//!
//! ## Accessibility
//!
//! [`render_icon`] emits `aria-hidden="true"` so decorative icons are excluded
//! from the screen-reader tree. Any control that is icon-only already carries
//! an `aria-label` on its host element, satisfying the requirement that icons
//! must never be the only accessible label.

use leptos::prelude::*;

// ── Re-exports ──────────────────────────────────────────────────
// PascalCase component functions exported by `lucide-leptos`. Re-exported here
// so ad-hoc imports stay consistent if a caller ever needs the raw component.
pub use lucide_leptos::Archive;
pub use lucide_leptos::ArrowDown;
pub use lucide_leptos::ArrowLeft;
pub use lucide_leptos::ArrowRight;
pub use lucide_leptos::ArrowUp;
pub use lucide_leptos::Bell;
pub use lucide_leptos::BellRing;
pub use lucide_leptos::BookCheck;
pub use lucide_leptos::BookMarked;
pub use lucide_leptos::BookOpen;
pub use lucide_leptos::BookText;
pub use lucide_leptos::Bookmark;
pub use lucide_leptos::Brush;
pub use lucide_leptos::Calendar;
pub use lucide_leptos::Camera;
pub use lucide_leptos::ChartBar;
pub use lucide_leptos::ChartColumn;
pub use lucide_leptos::ChartLine;
pub use lucide_leptos::ChartPie;
pub use lucide_leptos::Check;
pub use lucide_leptos::ChevronDown;
pub use lucide_leptos::ChevronLeft;
pub use lucide_leptos::ChevronRight;
pub use lucide_leptos::ChevronUp;
pub use lucide_leptos::ChevronsLeft;
pub use lucide_leptos::ChevronsRight;
pub use lucide_leptos::CircleAlert;
pub use lucide_leptos::CircleCheck;
pub use lucide_leptos::CircleEllipsis;
pub use lucide_leptos::CircleHelp;
pub use lucide_leptos::CircleX;
pub use lucide_leptos::Clipboard;
pub use lucide_leptos::ClipboardList;
pub use lucide_leptos::Clock;
pub use lucide_leptos::CloudUpload;
pub use lucide_leptos::Code;
pub use lucide_leptos::Columns2;
pub use lucide_leptos::Command as CommandIcon;
pub use lucide_leptos::Copy;
pub use lucide_leptos::CornerUpLeft;
pub use lucide_leptos::CornerUpRight;
pub use lucide_leptos::Download;
pub use lucide_leptos::Ellipsis;
pub use lucide_leptos::ExternalLink;
pub use lucide_leptos::Eye;
pub use lucide_leptos::EyeOff;
pub use lucide_leptos::File;
pub use lucide_leptos::FilePen;
pub use lucide_leptos::FilePlus;
pub use lucide_leptos::FileText;
pub use lucide_leptos::Files;
pub use lucide_leptos::Flame;
pub use lucide_leptos::Folder;
pub use lucide_leptos::FolderOpen;
pub use lucide_leptos::FolderTree;
pub use lucide_leptos::GalleryVerticalEnd;
pub use lucide_leptos::Globe;
pub use lucide_leptos::Grid2X2;
pub use lucide_leptos::History;
pub use lucide_leptos::Home;
pub use lucide_leptos::Inbox;
pub use lucide_leptos::Info;
pub use lucide_leptos::Kanban;
pub use lucide_leptos::Keyboard;
pub use lucide_leptos::Laptop;
pub use lucide_leptos::Layers;
pub use lucide_leptos::LayoutDashboard;
pub use lucide_leptos::Library;
pub use lucide_leptos::LifeBuoy;
pub use lucide_leptos::Lightbulb;
pub use lucide_leptos::Link;
pub use lucide_leptos::Link2;
pub use lucide_leptos::List;
pub use lucide_leptos::ListChecks;
pub use lucide_leptos::Loader;
pub use lucide_leptos::Mail;
pub use lucide_leptos::MapPin;
pub use lucide_leptos::Menu;
pub use lucide_leptos::MessageCircle;
pub use lucide_leptos::MessageSquare;
pub use lucide_leptos::Mic;
pub use lucide_leptos::Monitor;
pub use lucide_leptos::Moon;
pub use lucide_leptos::MoreHorizontal;
pub use lucide_leptos::MoreVertical;
pub use lucide_leptos::Network;
pub use lucide_leptos::NotebookPen;
pub use lucide_leptos::NotebookText;
pub use lucide_leptos::Package;
pub use lucide_leptos::Palette;
pub use lucide_leptos::PanelLeft;
pub use lucide_leptos::PanelRight;
pub use lucide_leptos::Paperclip;
pub use lucide_leptos::PenLine;
pub use lucide_leptos::Play;
pub use lucide_leptos::Plus;
pub use lucide_leptos::RefreshCcw;
pub use lucide_leptos::RefreshCw;
pub use lucide_leptos::Reply;
pub use lucide_leptos::ReplyAll;
pub use lucide_leptos::Save;
pub use lucide_leptos::ScanText;
pub use lucide_leptos::Scissors;
pub use lucide_leptos::Search;
pub use lucide_leptos::Send;
pub use lucide_leptos::Settings;
pub use lucide_leptos::Share;
pub use lucide_leptos::Share2;
pub use lucide_leptos::Slash;
pub use lucide_leptos::Smartphone;
pub use lucide_leptos::Sparkles;
pub use lucide_leptos::SquarePen;
pub use lucide_leptos::Star;
pub use lucide_leptos::StarHalf;
pub use lucide_leptos::StickyNote;
pub use lucide_leptos::Sun;
pub use lucide_leptos::Table;
pub use lucide_leptos::Tablet;
pub use lucide_leptos::Tag;
pub use lucide_leptos::Tags;
pub use lucide_leptos::Target;
pub use lucide_leptos::TextSearch;
pub use lucide_leptos::Trash2;
pub use lucide_leptos::TrendingDown;
pub use lucide_leptos::TrendingUp;
pub use lucide_leptos::TriangleAlert;
pub use lucide_leptos::Upload;
pub use lucide_leptos::Wand;
pub use lucide_leptos::Zap;
pub use lucide_leptos::X;

// ── Central icon enum ───────────────────────────────────────────

/// Every icon concept used by the Nabu UI.
///
/// This enum is the single source of truth for iconography. Components never
/// receive a raw emoji string; they receive an [`Icon`] (or `Option<Icon>`) and
/// render it via [`render_icon`].
///
/// Each variant maps to exactly one Lucide component. The variants are named
/// after the *concept* they represent (not the glyph) so the mapping table in
/// [`render_icon`] doubles as the migration report from the previous emoji set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    // Navigation & layout
    Dashboard,
    Editor,
    Graph,
    Inbox,
    ReadingQueue,
    Templates,
    Trash,
    History,
    Recovery,
    Calendar,
    Archive,
    SmartFolders,
    Settings,
    Search,
    SearchAlt,
    CommandPalette,
    QuickSwitcher,
    Shortcuts,
    DailyNote,
    NewNote,
    ToggleSidebar,
    ToggleInspector,
    Home,
    Back,
    Forward,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronsLeft,
    ChevronsRight,
    Menu,
    MoreHorizontal,
    MoreVertical,
    PanelLeft,
    PanelRight,
    // Files & notes
    Folder,
    FolderOpen,
    FolderTree,
    FileText,
    FilePen,
    FilePlus,
    StickyNote,
    NotebookPen,
    Save,
    Copy,
    Clipboard,
    // Status & feedback
    ToastSuccess,
    ToastWarning,
    ToastError,
    ToastInfo,
    Bell,
    BellRing,
    Warning,
    CircleAlert,
    Info,
    Check,
    CircleCheck,
    CircleX,
    X,
    Plus,
    Loader,
    RefreshCw,
    RefreshCcw,
    LifeBuoy,
    // Actions
    Star,
    StarHalf,
    Pin,
    Edit,
    Delete,
    // Communication
    Mail,
    Send,
    Share,
    Share2,
    Reply,
    ReplyAll,
    // Knowledge graph / links
    Network,
    Link,
    Link2,
    MessageCircle,
    MessageSquare,
    // Calendar & time
    Clock,
    Timer,
    // Views
    List,
    ListChecks,
    Grid,
    Columns,
    Table,
    Kanban,
    Gallery,
    Palette,
    Layers,
    Library,
    // Objects & tools
    Brush,
    Mic,
    Camera,
    Play,
    Package,
    Target,
    Command,
    Monitor,
    Smartphone,
    Tablet,
    Laptop,
    // Charts & stats
    ChartBar,
    ChartColumn,
    ChartLine,
    ChartPie,
    TrendingUp,
    TrendingDown,
    Flame,
    // Reader
    BookOpen,
    BookText,
    BookMarked,
    // Editor slash menu
    KanbanBoard,
    VisionOcr,
    CodeBlock,
    Callout,
    // Misc
    Sparkles,
    Wand,
    Globe,
    CloudUpload,
    Upload,
    Download,
    Scissors,
    ExternalLink,
    Sun,
    Moon,
    Ellipsis,
    CircleEllipsis,
    Eye,
    EyeOff,
    // Keyboard
    Keyboard,
}

impl Icon {
    /// Human-readable name, useful for debugging or ARIA labels where a
    /// textual description is required.
    pub fn name(self) -> &'static str {
        match self {
            // Navigation & layout
            Icon::Dashboard => "dashboard",
            Icon::Editor => "editor",
            Icon::Graph => "graph",
            Icon::Inbox => "inbox",
            Icon::ReadingQueue => "reading-queue",
            Icon::Templates => "templates",
            Icon::Trash => "trash",
            Icon::History => "history",
            Icon::Recovery => "recovery",
            Icon::Calendar => "calendar",
            Icon::Archive => "archive",
            Icon::SmartFolders => "smart-folders",
            Icon::Settings => "settings",
            Icon::Search => "search",
            Icon::SearchAlt => "search-alt",
            Icon::CommandPalette => "command-palette",
            Icon::QuickSwitcher => "quick-switcher",
            Icon::Shortcuts => "shortcuts",
            Icon::DailyNote => "daily-note",
            Icon::NewNote => "new-note",
            Icon::ToggleSidebar => "toggle-sidebar",
            Icon::ToggleInspector => "toggle-inspector",
            Icon::Home => "home",
            Icon::Back => "back",
            Icon::Forward => "forward",
            Icon::ChevronDown => "chevron-down",
            Icon::ChevronLeft => "chevron-left",
            Icon::ChevronRight => "chevron-right",
            Icon::ChevronUp => "chevron-up",
            Icon::ChevronsLeft => "chevrons-left",
            Icon::ChevronsRight => "chevrons-right",
            Icon::Menu => "menu",
            Icon::MoreHorizontal => "more-horizontal",
            Icon::MoreVertical => "more-vertical",
            Icon::PanelLeft => "panel-left",
            Icon::PanelRight => "panel-right",
            // Files & notes
            Icon::Folder => "folder",
            Icon::FolderOpen => "folder-open",
            Icon::FolderTree => "folder-tree",
            Icon::FileText => "file-text",
            Icon::FilePen => "file-pen",
            Icon::FilePlus => "file-plus",
            Icon::StickyNote => "sticky-note",
            Icon::NotebookPen => "notebook-pen",
            Icon::Save => "save",
            Icon::Copy => "copy",
            Icon::Clipboard => "clipboard",
            // Status & feedback
            Icon::ToastSuccess => "toast-success",
            Icon::ToastWarning => "toast-warning",
            Icon::ToastError => "toast-error",
            Icon::ToastInfo => "toast-info",
            Icon::Bell => "bell",
            Icon::BellRing => "bell-ring",
            Icon::Warning => "warning",
            Icon::CircleAlert => "circle-alert",
            Icon::Info => "info",
            Icon::Check => "check",
            Icon::CircleCheck => "circle-check",
            Icon::CircleX => "circle-x",
            Icon::X => "x",
            Icon::Plus => "plus",
            Icon::Loader => "loader",
            Icon::RefreshCw => "refresh-cw",
            Icon::RefreshCcw => "refresh-ccw",
            Icon::LifeBuoy => "life-buoy",
            // Actions
            Icon::Star => "star",
            Icon::StarHalf => "star-half",
            Icon::Pin => "pin",
            Icon::Edit => "edit",
            Icon::Delete => "delete",
            // Communication
            Icon::Mail => "mail",
            Icon::Send => "send",
            Icon::Share => "share",
            Icon::Share2 => "share-2",
            Icon::Reply => "reply",
            Icon::ReplyAll => "reply-all",
            // Knowledge graph / links
            Icon::Network => "network",
            Icon::Link => "link",
            Icon::Link2 => "link-2",
            Icon::MessageCircle => "message-circle",
            Icon::MessageSquare => "message-square",
            // Calendar & time
            Icon::Clock => "clock",
            Icon::Timer => "timer",
            // Views
            Icon::List => "list",
            Icon::ListChecks => "list-checks",
            Icon::Grid => "grid",
            Icon::Columns => "columns",
            Icon::Table => "table",
            Icon::Kanban => "kanban",
            Icon::Gallery => "gallery",
            Icon::Palette => "palette",
            Icon::Layers => "layers",
            Icon::Library => "library",
            // Objects & tools
            Icon::Brush => "brush",
            Icon::Mic => "mic",
            Icon::Camera => "camera",
            Icon::Play => "play",
            Icon::Package => "package",
            Icon::Target => "target",
            Icon::Command => "command",
            Icon::Monitor => "monitor",
            Icon::Smartphone => "smartphone",
            Icon::Tablet => "tablet",
            Icon::Laptop => "laptop",
            // Charts & stats
            Icon::ChartBar => "chart-bar",
            Icon::ChartColumn => "chart-column",
            Icon::ChartLine => "chart-line",
            Icon::ChartPie => "chart-pie",
            Icon::TrendingUp => "trending-up",
            Icon::TrendingDown => "trending-down",
            Icon::Flame => "flame",
            // Reader
            Icon::BookOpen => "book-open",
            Icon::BookText => "book-text",
            Icon::BookMarked => "book-marked",
            // Editor slash menu
            Icon::KanbanBoard => "kanban-board",
            Icon::VisionOcr => "vision-ocr",
            Icon::CodeBlock => "code-block",
            Icon::Callout => "callout",
            // Misc
            Icon::Sparkles => "sparkles",
            Icon::Wand => "wand",
            Icon::Globe => "globe",
            Icon::CloudUpload => "cloud-upload",
            Icon::Upload => "upload",
            Icon::Download => "download",
            Icon::Scissors => "scissors",
            Icon::ExternalLink => "external-link",
            Icon::Sun => "sun",
            Icon::Moon => "moon",
            Icon::Ellipsis => "ellipsis",
            Icon::CircleEllipsis => "circle-ellipsis",
            Icon::Eye => "eye",
            Icon::EyeOff => "eye-off",
            // Keyboard
            Icon::Keyboard => "keyboard",
        }
    }
}

/// Render an [`Icon`] as a theme-inheriting, accessible Lucide SVG.
///
/// The SVG scales with the parent's `font-size` (via the global `.lucide`
/// rule) so it occupies approximately the same visual footprint as the emoji
/// it replaces — call sites keep their existing `text-*` / `text-[...]`
/// sizing classes on the wrapping element.
///
/// `aria-hidden="true"` keeps decorative icons out of the screen-reader tree;
/// icon-only controls that use this helper already carry an `aria-label` on
/// their host element, so an icon is never the only accessible label.
#[component]
pub fn render_icon(
    /// Which icon to draw.
    icon: Icon,
    /// Optional extra classes applied to the wrapping `<span>` (e.g. sizing).
    #[prop(optional)]
    class: Option<&'static str>,
) -> impl IntoView {
    let cls = move || match class {
        Some(extra) => format!("inline-flex items-center justify-center {extra}"),
        None => String::from("inline-flex items-center justify-center"),
    };
    view! {
        <span class=cls aria-hidden="true">
            {icon_component(icon)}
        </span>
    }
}

/// Resolve an [`Icon`] to its underlying Lucide component view. This is the
/// single dispatch table: change a glyph here and it updates everywhere.
fn icon_component(icon: Icon) -> AnyView {
    macro_rules! ic {
        ($cmp:ident) => {
            view! { <$cmp /> }.into_any()
        };
    }
    match icon {
        // ── Navigation & layout ───────────────────────────
        Icon::Dashboard => ic!(LayoutDashboard),
        Icon::Editor => ic!(NotebookText),
        Icon::Graph => ic!(Network),
        Icon::Inbox => ic!(Inbox),
        Icon::ReadingQueue => ic!(BookCheck),
        Icon::Templates => ic!(ClipboardList),
        Icon::Trash => ic!(Trash2),
        Icon::History => ic!(History),
        Icon::Recovery => ic!(LifeBuoy),
        Icon::Calendar => ic!(Calendar),
        Icon::Archive => ic!(Archive),
        Icon::SmartFolders => ic!(FolderTree),
        Icon::Settings => ic!(Settings),
        Icon::Search => ic!(Search),
        Icon::SearchAlt => ic!(TextSearch),
        Icon::CommandPalette => ic!(CommandIcon),
        Icon::QuickSwitcher => ic!(Zap),
        Icon::Shortcuts => ic!(Keyboard),
        Icon::DailyNote => ic!(Calendar),
        Icon::NewNote => ic!(Plus),
        Icon::ToggleSidebar => ic!(PanelLeft),
        Icon::ToggleInspector => ic!(ClipboardList),
        Icon::Home => ic!(Home),
        Icon::Back => ic!(ArrowLeft),
        Icon::Forward => ic!(ArrowRight),
        Icon::ChevronDown => ic!(ChevronDown),
        Icon::ChevronLeft => ic!(ChevronLeft),
        Icon::ChevronRight => ic!(ChevronRight),
        Icon::ChevronUp => ic!(ChevronUp),
        Icon::ChevronsLeft => ic!(ChevronsLeft),
        Icon::ChevronsRight => ic!(ChevronsRight),
        Icon::Menu => ic!(Menu),
        Icon::MoreHorizontal => ic!(MoreHorizontal),
        Icon::MoreVertical => ic!(MoreVertical),
        Icon::PanelLeft => ic!(PanelLeft),
        Icon::PanelRight => ic!(PanelRight),
        // ── Files & notes ───────────────────────────────
        Icon::Folder => ic!(Folder),
        Icon::FolderOpen => ic!(FolderOpen),
        Icon::FolderTree => ic!(FolderTree),
        Icon::FileText => ic!(FileText),
        Icon::FilePen => ic!(FilePen),
        Icon::FilePlus => ic!(FilePlus),
        Icon::StickyNote => ic!(StickyNote),
        Icon::NotebookPen => ic!(NotebookPen),
        Icon::Save => ic!(Save),
        Icon::Copy => ic!(Copy),
        Icon::Clipboard => ic!(Clipboard),
        // ── Status & feedback ───────────────────────────
        Icon::ToastSuccess => ic!(CircleCheck),
        Icon::ToastWarning => ic!(TriangleAlert),
        Icon::ToastError => ic!(CircleX),
        Icon::ToastInfo => ic!(Info),
        Icon::Bell => ic!(Bell),
        Icon::BellRing => ic!(BellRing),
        Icon::Warning => ic!(TriangleAlert),
        Icon::CircleAlert => ic!(CircleAlert),
        Icon::Info => ic!(Info),
        Icon::Check => ic!(Check),
        Icon::CircleCheck => ic!(CircleCheck),
        Icon::CircleX => ic!(CircleX),
        Icon::X => ic!(X),
        Icon::Plus => ic!(Plus),
        Icon::Loader => ic!(Loader),
        Icon::RefreshCw => ic!(RefreshCw),
        Icon::RefreshCcw => ic!(RefreshCcw),
        Icon::LifeBuoy => ic!(LifeBuoy),
        // ── Actions ─────────────────────────────────────
        Icon::Star => ic!(Star),
        Icon::StarHalf => ic!(StarHalf),
        Icon::Pin => ic!(MapPin),
        Icon::Edit => ic!(PenLine),
        Icon::Delete => ic!(Trash2),
        // ── Communication ───────────────────────────────
        Icon::Mail => ic!(Mail),
        Icon::Send => ic!(Send),
        Icon::Share => ic!(Share),
        Icon::Share2 => ic!(Share2),
        Icon::Reply => ic!(Reply),
        Icon::ReplyAll => ic!(ReplyAll),
        // ── Knowledge graph / links ─────────────────────
        Icon::Network => ic!(Network),
        Icon::Link => ic!(Link),
        Icon::Link2 => ic!(Link2),
        Icon::MessageCircle => ic!(MessageCircle),
        Icon::MessageSquare => ic!(MessageSquare),
        // ── Calendar & time ─────────────────────────────
        Icon::Clock => ic!(Clock),
        Icon::Timer => ic!(Clock),
        // ── Views ───────────────────────────────────────
        Icon::List => ic!(List),
        Icon::ListChecks => ic!(ListChecks),
        Icon::Grid => ic!(Grid2X2),
        Icon::Columns => ic!(Columns2),
        Icon::Table => ic!(Table),
        Icon::Kanban => ic!(Kanban),
        Icon::Gallery => ic!(GalleryVerticalEnd),
        Icon::Palette => ic!(Palette),
        Icon::Layers => ic!(Layers),
        Icon::Library => ic!(Library),
        // ── Objects & tools ─────────────────────────────
        Icon::Brush => ic!(Brush),
        Icon::Mic => ic!(Mic),
        Icon::Camera => ic!(Camera),
        Icon::Play => ic!(Play),
        Icon::Package => ic!(Package),
        Icon::Target => ic!(Target),
        Icon::Command => ic!(CommandIcon),
        Icon::Monitor => ic!(Monitor),
        Icon::Smartphone => ic!(Smartphone),
        Icon::Tablet => ic!(Tablet),
        Icon::Laptop => ic!(Laptop),
        // ── Charts & stats ──────────────────────────────
        Icon::ChartBar => ic!(ChartBar),
        Icon::ChartColumn => ic!(ChartColumn),
        Icon::ChartLine => ic!(ChartLine),
        Icon::ChartPie => ic!(ChartPie),
        Icon::TrendingUp => ic!(TrendingUp),
        Icon::TrendingDown => ic!(TrendingDown),
        Icon::Flame => ic!(Flame),
        // ── Reader ──────────────────────────────────────
        Icon::BookOpen => ic!(BookOpen),
        Icon::BookText => ic!(BookText),
        Icon::BookMarked => ic!(BookMarked),
        // ── Editor slash menu ───────────────────────────
        Icon::KanbanBoard => ic!(Kanban),
        Icon::VisionOcr => ic!(ScanText),
        Icon::CodeBlock => ic!(Code),
        Icon::Callout => ic!(Lightbulb),
        // ── Misc ────────────────────────────────────────
        Icon::Sparkles => ic!(Sparkles),
        Icon::Wand => ic!(Wand),
        Icon::Globe => ic!(Globe),
        Icon::CloudUpload => ic!(CloudUpload),
        Icon::Upload => ic!(Upload),
        Icon::Download => ic!(Download),
        Icon::Scissors => ic!(Scissors),
        Icon::ExternalLink => ic!(ExternalLink),
        Icon::Sun => ic!(Sun),
        Icon::Moon => ic!(Moon),
        Icon::Ellipsis => ic!(Ellipsis),
        Icon::CircleEllipsis => ic!(CircleEllipsis),
        Icon::Eye => ic!(Eye),
        Icon::EyeOff => ic!(EyeOff),
        // ── Keyboard ────────────────────────────────────
        Icon::Keyboard => ic!(Keyboard),
    }
}

/// Convenience wrapper: render an [`Icon`] inside a sized, theme-coloured span.
/// This mirrors the previous emoji pattern (`<span class="…">{emoji}</span>`)
/// so call sites can drop a single component in place of a text emoji node.
///
/// When `label` is `None` the icon is decorative (`aria-hidden="true"`); when
/// `Some` the icon carries meaning and the label is exposed to assistive
/// technology via a `title`.
#[component]
pub fn IconEl(
    /// Which icon to draw.
    icon: Icon,
    /// Optional extra Tailwind/utility classes (e.g. sizing).
    #[prop(optional)]
    class: Option<&'static str>,
    /// Optional accessible label. When `None` the icon is decorative.
    #[prop(optional)]
    label: Option<&'static str>,
) -> impl IntoView {
    let aria = label.is_none();
    view! {
        <span
            class=move || format!("{}{}", "inline-flex items-center justify-center ", class.unwrap_or(""))
            aria-hidden=aria
            title=label
        >
            {render_icon(icon, class)}
        </span>
    }
}
