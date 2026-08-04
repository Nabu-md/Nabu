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
pub use lucide_leptos::Circle;
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
pub use lucide_leptos::EllipsisVertical;
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
pub use lucide_leptos::Globe;
pub use lucide_leptos::Grid2X2;
pub use lucide_leptos::HardDrive;
pub use lucide_leptos::Hash;
pub use lucide_leptos::History;
pub use lucide_leptos::House;
pub use lucide_leptos::Image;
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
pub use lucide_leptos::Music;
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
pub use lucide_leptos::Redo;
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
pub use lucide_leptos::Undo;
pub use lucide_leptos::Upload;
pub use lucide_leptos::User;
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
    Trash2,
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
    File,
    FileText,
    FilePen,
    FilePlus,
    StickyNote,
    NotebookText,
    NotebookPen,
    BookCheck,
    BookOpen,
    BookText,
    BookMarked,
    Save,
    Copy,
    Clipboard,
    ClipboardList,
    Tag,
    PenLine,
    Image,    // Status & feedback
    ToastSuccess,
    ToastWarning,
    ToastError,
    ToastInfo,
    Bell,
    BellRing,
    Warning,
    CircleAlert,
    Circle,
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
    Undo,
    Redo,
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
    Comparison,
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
    Music,
    Play,
    Package,
    Target,
    Command,
    Monitor,
    Smartphone,
    Tablet,
    Laptop,
    User,
    MapPin,
    HardDrive,
    // Charts & stats
    ChartBar,
    ChartColumn,
    ChartLine,
    ChartPie,
    TrendingUp,
    TrendingDown,
    Flame,
    // Editor slash menu
    KanbanBoard,
    VisionOcr,
    CodeBlock,
    Callout,
    // Misc
    Sparkles,
    Wand,
    Zap,
    Globe,
    GalleryVerticalEnd,
    CircleHelp,
    Bookmark,
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
            Icon::Trash2 => "trash-2",
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
            Icon::File => "file",
            Icon::FileText => "file-text",
            Icon::FilePen => "file-pen",
            Icon::FilePlus => "file-plus",
            Icon::StickyNote => "sticky-note",
            Icon::NotebookText => "notebook-text",
            Icon::NotebookPen => "notebook-pen",
            Icon::BookCheck => "book-check",
            Icon::BookOpen => "book-open",
            Icon::BookText => "book-text",
            Icon::BookMarked => "book-marked",
            Icon::Save => "save",
            Icon::Copy => "copy",
            Icon::Clipboard => "clipboard",
            Icon::ClipboardList => "clipboard-list",
            Icon::Tag => "tag",
            Icon::PenLine => "pen-line",
            Icon::Image => "image",
            // Status & feedback
            Icon::ToastSuccess => "toast-success",
            Icon::ToastWarning => "toast-warning",
            Icon::ToastError => "toast-error",
            Icon::ToastInfo => "toast-info",
            Icon::Bell => "bell",
            Icon::BellRing => "bell-ring",
            Icon::Warning => "warning",
            Icon::CircleAlert => "circle-alert",
            Icon::Circle => "circle",
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
            Icon::Undo => "undo",
            Icon::Redo => "redo",
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
            Icon::Comparison => "comparison",
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
            Icon::Music => "music",
            Icon::Play => "play",
            Icon::Package => "package",
            Icon::Target => "target",
                           Icon::Command => "command",
            Icon::Monitor => "monitor",
            Icon::Smartphone => "smartphone",
            Icon::Tablet => "tablet",
            Icon::Laptop => "laptop",
            Icon::User => "user",
            Icon::MapPin => "map-pin",
            Icon::HardDrive => "hard-drive",
            // Charts & stats
            Icon::ChartBar => "chart-bar",
            Icon::ChartColumn => "chart-column",
            Icon::ChartLine => "chart-line",
            Icon::ChartPie => "chart-pie",
            Icon::TrendingUp => "trending-up",
            Icon::TrendingDown => "trending-down",
            Icon::Flame => "flame",
            // Editor slash menu
            Icon::KanbanBoard => "kanban-board",
            Icon::VisionOcr => "vision-ocr",
            Icon::CodeBlock => "code-block",
            Icon::Callout => "callout",
            // Misc
            Icon::Sparkles => "sparkles",
            Icon::Zap => "zap",
            Icon::Wand => "wand",
            Icon::Globe => "globe",
            Icon::GalleryVerticalEnd => "gallery-vertical-end",
            Icon::CircleHelp => "circle-help",
            Icon::Bookmark => "bookmark",
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

/// Resolve an [`Icon`] to its underlying Lucide component view. This is the
/// single dispatch table: change a glyph here and it updates everywhere.
///
/// Each arm invokes the real Lucide component (a compile-time SVG) so there is
/// no runtime icon loading.
fn icon_component(icon: Icon) -> AnyView {
    // Lucide component names live in this module's scope via the re-exports
    // above. The `view!` macro resolves each `<Name />` to the component and
    // applies all its default props (size=24, color=currentColor).
    macro_rules! c {
        ($cmp:ident) => {
            view! { <$cmp /> }.into_any()
        };
    }
    match icon {
        // ── Navigation & layout ───────────────────────────
        Icon::Dashboard => c!(LayoutDashboard),
        Icon::Editor => c!(NotebookText),
        Icon::Graph => c!(Network),
        Icon::Inbox => c!(Inbox),
        Icon::ReadingQueue => c!(BookCheck),
        Icon::Templates => c!(ClipboardList),
        Icon::Trash => c!(Trash2),
        Icon::Trash2 => c!(Trash2),
        Icon::History => c!(History),
        Icon::Recovery => c!(LifeBuoy),
        Icon::Calendar => c!(Calendar),
        Icon::Archive => c!(Archive),
        Icon::SmartFolders => c!(FolderTree),
        Icon::Settings => c!(Settings),
        Icon::Search => c!(Search),
        Icon::SearchAlt => c!(TextSearch),
        Icon::CommandPalette => c!(CommandIcon),
        Icon::QuickSwitcher => c!(Zap),
        Icon::Shortcuts => c!(Keyboard),
        Icon::DailyNote => c!(Calendar),
        Icon::NewNote => c!(Plus),
        Icon::ToggleSidebar => c!(PanelLeft),
        Icon::ToggleInspector => c!(ClipboardList),
        Icon::Home => c!(House),
        Icon::Back => c!(ArrowLeft),
        Icon::Forward => c!(ArrowRight),
        Icon::ChevronDown => c!(ChevronDown),
        Icon::ChevronLeft => c!(ChevronLeft),
        Icon::ChevronRight => c!(ChevronRight),
        Icon::ChevronUp => c!(ChevronUp),
        Icon::ChevronsLeft => c!(ChevronsLeft),
        Icon::ChevronsRight => c!(ChevronsRight),
        Icon::Menu => c!(Menu),
        Icon::MoreHorizontal => c!(Ellipsis),
        Icon::MoreVertical => c!(EllipsisVertical),
        Icon::PanelLeft => c!(PanelLeft),
        Icon::PanelRight => c!(PanelRight),
        // ── Files & notes ───────────────────────────────
        Icon::Folder => c!(Folder),
        Icon::FolderOpen => c!(FolderOpen),
        Icon::FolderTree => c!(FolderTree),
        Icon::File => c!(File),
        Icon::FileText => c!(FileText),
        Icon::FilePen => c!(FilePen),
        Icon::FilePlus => c!(FilePlus),
        Icon::StickyNote => c!(StickyNote),
        Icon::NotebookText => c!(NotebookText),
        Icon::NotebookPen => c!(NotebookPen),
        Icon::BookCheck => c!(BookCheck),
        Icon::BookOpen => c!(BookOpen),
        Icon::BookText => c!(BookText),
        Icon::BookMarked => c!(BookMarked),
        Icon::Save => c!(Save),
        Icon::Copy => c!(Copy),
        Icon::Clipboard => c!(Clipboard),
        Icon::ClipboardList => c!(ClipboardList),
        Icon::Tag => c!(Tag),
        Icon::PenLine => c!(PenLine),
        Icon::Image => c!(Image),
        // ── Status & feedback ───────────────────────────
        Icon::ToastSuccess => c!(CircleCheck),
        Icon::ToastWarning => c!(TriangleAlert),
        Icon::ToastError => c!(CircleX),
        Icon::ToastInfo => c!(Info),
        Icon::Bell => c!(Bell),
        Icon::BellRing => c!(BellRing),
        Icon::Warning => c!(TriangleAlert),
        Icon::CircleAlert => c!(CircleAlert),
        Icon::Circle => c!(Circle),
        Icon::Info => c!(Info),
        Icon::Check => c!(Check),
        Icon::CircleCheck => c!(CircleCheck),
        Icon::CircleX => c!(CircleX),
        Icon::X => c!(X),
        Icon::Plus => c!(Plus),
        Icon::Loader => c!(Loader),
        Icon::RefreshCw => c!(RefreshCw),
        Icon::RefreshCcw => c!(RefreshCcw),
        Icon::LifeBuoy => c!(LifeBuoy),
        // ── Actions ─────────────────────────────────────
        Icon::Star => c!(Star),
        Icon::StarHalf => c!(StarHalf),
        Icon::Pin => c!(MapPin),
        Icon::Edit => c!(PenLine),
        Icon::Delete => c!(Trash2),
        Icon::Undo => c!(Undo),
        Icon::Redo => c!(Redo),
        // ── Communication ───────────────────────────────
        Icon::Mail => c!(Mail),
        Icon::Send => c!(Send),
        Icon::Share => c!(Share),
        Icon::Share2 => c!(Share2),
        Icon::Reply => c!(Reply),
        Icon::ReplyAll => c!(ReplyAll),
        // ── Knowledge graph / links ─────────────────────
        Icon::Network => c!(Network),
        Icon::Link => c!(Link),
        Icon::Link2 => c!(Link2),
        Icon::MessageCircle => c!(MessageCircle),
        Icon::MessageSquare => c!(MessageSquare),
        Icon::Comparison => c!(GitCompare),
        // ── Calendar & time ─────────────────────────────
        Icon::Clock => c!(Clock),
        Icon::Timer => c!(Clock),
        // ── Views ───────────────────────────────────────
        Icon::List => c!(List),
        Icon::ListChecks => c!(ListChecks),
        Icon::Grid => c!(Grid2X2),
        Icon::Columns => c!(Columns2),
        Icon::Table => c!(Table),
        Icon::Kanban => c!(Kanban),
        Icon::Gallery => c!(GalleryVerticalEnd),
        Icon::GalleryVerticalEnd => c!(GalleryVerticalEnd),
        Icon::CircleHelp => c!(CircleHelp),
        Icon::Bookmark => c!(Bookmark),
        Icon::Palette => c!(Palette),
        Icon::Layers => c!(Layers),
        Icon::Library => c!(Library),
        // ── Objects & tools ─────────────────────────────
        Icon::Brush => c!(Brush),
        Icon::Mic => c!(Mic),
        Icon::Camera => c!(Camera),
        Icon::Music => c!(Music),
        Icon::Play => c!(Play),
        Icon::Package => c!(Package),
        Icon::Target => c!(Target),
        Icon::Command => c!(CommandIcon),
        Icon::Monitor => c!(Monitor),
        Icon::Smartphone => c!(Smartphone),
        Icon::Tablet => c!(Tablet),
        Icon::Laptop => c!(Laptop),
        Icon::User => c!(User),
        Icon::MapPin => c!(MapPin),
        Icon::HardDrive => c!(HardDrive),
        // ── Charts & stats ──────────────────────────────
        Icon::ChartBar => c!(ChartBar),
        Icon::ChartColumn => c!(ChartColumn),
        Icon::ChartLine => c!(ChartLine),
        Icon::ChartPie => c!(ChartPie),
        Icon::TrendingUp => c!(TrendingUp),
        Icon::TrendingDown => c!(TrendingDown),
        Icon::Flame => c!(Flame),
        // ── Editor slash menu ───────────────────────────        // ── Editor slash menu ───────────────────────────
        Icon::KanbanBoard => c!(Kanban),
        Icon::VisionOcr => c!(ScanText),
        Icon::CodeBlock => c!(Code),
        Icon::Callout => c!(Lightbulb),
        // ── Misc ────────────────────────────────────────
        Icon::Sparkles => c!(Sparkles),
        Icon::Zap => c!(Zap),
        Icon::Wand => c!(Wand),
        Icon::Globe => c!(Globe),
        Icon::CloudUpload => c!(CloudUpload),
        Icon::Upload => c!(Upload),
        Icon::Download => c!(Download),
        Icon::Scissors => c!(Scissors),
        Icon::ExternalLink => c!(ExternalLink),
        Icon::Sun => c!(Sun),
        Icon::Moon => c!(Moon),
        Icon::Ellipsis => c!(Ellipsis),
        Icon::CircleEllipsis => c!(CircleEllipsis),
        Icon::Eye => c!(Eye),
        Icon::EyeOff => c!(EyeOff),
        // ── Keyboard ────────────────────────────────────
        Icon::Keyboard => c!(Keyboard),
    }
}

/// Plain function form of the icon renderer — returns a themed SVG view
/// directly (no wrapping span), for use inside other `view!` blocks as
/// `{render_icon_view(Icon::Foo)}`.
pub fn render_icon_view(icon: Icon) -> AnyView {
    icon_component(icon)
}

/// Render an [`Icon`] inside an optional wrapping `<span>` so callers can drop
/// it in place of a text emoji node inside a `view!` block:
/// `{render_icon(Icon::Search, Some("text-2xl"))}`.
///
/// The wrapping span carries `aria-hidden="true"` so decorative icons are
/// excluded from the screen-reader tree. Icon-only controls that use this
/// helper already carry an `aria-label` on their host element.
///
/// The SVG scales with the parent's `font-size` (via the global `.lucide`
/// CSS rule) so existing `text-*` / `text-[...]` sizing classes still control
/// the icon footprint, exactly like the emoji glyphs they replace.
pub fn render_icon(icon: Icon, class: Option<&'static str>) -> AnyView {
    match class {
        Some(extra) => view! {
            <span class=format!("lucide-icon {extra}") aria-hidden="true">
                {icon_component(icon)}
            </span>
        }
        .into_any(),
        None => view! {
            <span class="lucide-icon" aria-hidden="true">
                {icon_component(icon)}
            </span>
        }
        .into_any(),
    }
}

/// Convenience wrapper: render an [`Icon`] inside a sized, theme-coloured span.
/// Mirrors the previous emoji pattern (`<span class="…">{emoji}</span>`).
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
            class=move || format!("{}{}", "lucide-icon ", class.unwrap_or(""))
            aria-hidden=aria
            title=label
        >
            {icon_component(icon)}
        </span>
    }
}
