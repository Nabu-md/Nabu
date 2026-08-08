//! # Nabu Icon System (Dioxus)
//!
//! Centralized icon abstraction for every Lucide icon used in the
//! application. Icons are rendered as inline SVG with `stroke="currentColor"`
//! so they inherit the surrounding text colour and adapt to Light / Dark /
//! System themes via CSS `data-theme`.
//!
//! The [`Icon`] enum is the single source of truth for *which* icon appears
//! wherever a UI element needs an icon. Call sites construct an [`Icon`]
//! variant and hand it to [`render_icon`].
//!
//! SVG path data is sourced from `lucide-leptos` 0.2.0 (same data the Leptos
//! build used), embedded as inline SVG so no Leptos runtime is required.

use dioxus::prelude::*;

// ── Central icon enum ───────────────────────────────────────────

/// Every icon concept used by the Nabu UI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    // Navigation & layout
    Dashboard, Editor, Graph, Inbox, ReadingQueue, Templates,
    Trash, Trash2, History, Recovery, Calendar, Archive,
    SmartFolders, Settings, Search, SearchAlt, CommandPalette,
    QuickSwitcher, Shortcuts, DailyNote, NewNote, ToggleSidebar,
    ToggleInspector, Home, Back, Forward, ChevronDown, ChevronLeft,
    ChevronRight, ChevronUp, ChevronsLeft, ChevronsRight, Menu,
    MoreHorizontal, MoreVertical, PanelLeft, PanelRight,
    // Files & notes
    Folder, FolderOpen, FolderTree, File, FileText, FilePen, FilePlus,
    StickyNote, NotebookText, NotebookPen, BookCheck, BookOpen, BookText,
    BookMarked, Save, Copy, Clipboard, ClipboardList, Tag, PenLine, Image,
    // Status & feedback
    ToastSuccess, ToastWarning, ToastError, ToastInfo, Bell, BellRing,
    Warning, CircleAlert, Circle, Info, Check, CircleCheck, CircleX, X,
    Plus, Loader, RefreshCw, RefreshCcw, LifeBuoy,
    // Actions
    Star, StarHalf, Pin, Edit, Delete, Undo, Redo,
    // Communication
    Mail, Send, Share, Share2, Reply, ReplyAll,
    // Knowledge graph / links
    Network, Link, Link2, MessageCircle, MessageSquare, Comparison,
    // Calendar & time
    Clock, Timer,
    // Views
    List, ListChecks, Grid, Columns, Table, Kanban, Gallery, Palette,
    Layers, Library,
    // Objects & tools
    Brush, Mic, Camera, Music, Play, Package, Target, Command, Monitor,
    Smartphone, Tablet, Laptop, User, MapPin, HardDrive,
    // Charts & stats
    ChartBar, ChartColumn, ChartLine, ChartPie, TrendingUp, TrendingDown,
    Flame,
    // Editor slash menu
    KanbanBoard, VisionOcr, CodeBlock, Callout,
    // Misc
    Sparkles, Zap, Wand, Globe, GalleryVerticalEnd, CircleHelp, Bookmark,
     CloudUpload, Upload, Download, Scissors, ExternalLink, Sun, Moon,
     Ellipsis, CircleEllipsis, Eye, EyeOff,
     // Activity / timeline
     Activity,
     // Keyboard
     Keyboard,
    // Additional re-exported icons
    GitCompare, Slash,
}

impl Icon {
    pub fn name(self) -> &'static str {
        match self {

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
        Icon::Star => "star",
        Icon::StarHalf => "star-half",
        Icon::Pin => "pin",
        Icon::Edit => "edit",
        Icon::Delete => "delete",
        Icon::Undo => "undo",
        Icon::Redo => "redo",
        Icon::Mail => "mail",
        Icon::Send => "send",
        Icon::Share => "share",
        Icon::Share2 => "share-2",
        Icon::Reply => "reply",
        Icon::ReplyAll => "reply-all",
        Icon::Network => "network",
        Icon::Link => "link",
        Icon::Link2 => "link-2",
        Icon::MessageCircle => "message-circle",
        Icon::MessageSquare => "message-square",
        Icon::Comparison => "comparison",
        Icon::Clock => "clock",
        Icon::Timer => "timer",
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
        Icon::ChartBar => "chart-bar",
        Icon::ChartColumn => "chart-column",
        Icon::ChartLine => "chart-line",
        Icon::ChartPie => "chart-pie",
        Icon::TrendingUp => "trending-up",
        Icon::TrendingDown => "trending-down",
        Icon::Flame => "flame",
        Icon::KanbanBoard => "kanban-board",
        Icon::VisionOcr => "vision-ocr",
        Icon::CodeBlock => "code-block",
        Icon::Callout => "callout",
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
         Icon::Activity => "activity",
         Icon::Keyboard => "keyboard",
        Icon::GitCompare => "git-compare",
        Icon::Slash => "slash",
        }
    }
}

// ── SVG rendering ─────────────────────────────────────────────────

/// Resolve an [`Icon`] to its underlying SVG view. This is the single
/// dispatch table: change a glyph here and it updates everywhere.
pub fn icon_component(icon: Icon) -> Element {
    match icon {

        Icon::Dashboard => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "7", height: "9", x: "3", y: "3", rx: "1" }
                    rect { width: "7", height: "5", x: "14", y: "3", rx: "1" }
                    rect { width: "7", height: "9", x: "14", y: "12", rx: "1" }
                    rect { width: "7", height: "5", x: "3", y: "16", rx: "1" }
                }
            }
        }
        Icon::Editor => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M2 6h4" }
                    path { d: "M2 10h4" }
                    path { d: "M2 14h4" }
                    path { d: "M2 18h4" }
                    rect { width: "16", height: "20", x: "4", y: "2", rx: "2" }
                    path { d: "M9.5 8h5" }
                    path { d: "M9.5 12H16" }
                    path { d: "M9.5 16H14" }
                }
            }
        }
        Icon::Graph => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { x: "16", y: "16", width: "6", height: "6", rx: "1" }
                    rect { x: "2", y: "16", width: "6", height: "6", rx: "1" }
                    rect { x: "9", y: "2", width: "6", height: "6", rx: "1" }
                    path { d: "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3" }
                    path { d: "M12 12V8" }
                }
            }
        }
        Icon::Inbox => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "22 12 16 12 14 15 10 15 8 12 2 12" }
                    path { d: "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" }
                }
            }
        }
        Icon::ReadingQueue => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20" }
                    path { d: "m9 9.5 2 2 4-4" }
                }
            }
        }
        Icon::Templates => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "8", height: "4", x: "8", y: "2", rx: "1", ry: "1" }
                    path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
                    path { d: "M12 11h4" }
                    path { d: "M12 16h4" }
                    path { d: "M8 11h.01" }
                    path { d: "M8 16h.01" }
                }
            }
        }
        Icon::Trash => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 6h18" }
                    path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
                    path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
                    line { x1: "10", x2: "10", y1: "11", y2: "17" }
                    line { x1: "14", x2: "14", y1: "11", y2: "17" }
                }
            }
        }
        Icon::Trash2 => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 6h18" }
                    path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
                    path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
                    line { x1: "10", x2: "10", y1: "11", y2: "17" }
                    line { x1: "14", x2: "14", y1: "11", y2: "17" }
                }
            }
        }
        Icon::History => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                    path { d: "M3 3v5h5" }
                    path { d: "M12 7v5l4 2" }
                }
            }
        }
        Icon::Recovery => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m4.93 4.93 4.24 4.24" }
                    path { d: "m14.83 9.17 4.24-4.24" }
                    path { d: "m14.83 14.83 4.24 4.24" }
                    path { d: "m9.17 14.83-4.24 4.24" }
                    circle { cx: "12", cy: "12", r: "4" }
                }
            }
        }
        Icon::Calendar => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M8 2v4" }
                    path { d: "M16 2v4" }
                    rect { width: "18", height: "18", x: "3", y: "4", rx: "2" }
                    path { d: "M3 10h18" }
                }
            }
        }
        Icon::Archive => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "20", height: "5", x: "2", y: "3", rx: "1" }
                    path { d: "M4 8v11a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8" }
                    path { d: "M10 12h4" }
                }
            }
        }
        Icon::SmartFolders => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z" }
                    path { d: "M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z" }
                    path { d: "M3 5a2 2 0 0 0 2 2h3" }
                    path { d: "M3 3v13a2 2 0 0 0 2 2h3" }
                }
            }
        }
        Icon::Settings => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
                    circle { cx: "12", cy: "12", r: "3" }
                }
            }
        }
        Icon::Search => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "11", cy: "11", r: "8" }
                    path { d: "m21 21-4.3-4.3" }
                }
            }
        }
        Icon::SearchAlt => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 6H3" }
                    path { d: "M10 12H3" }
                    path { d: "M10 18H3" }
                    circle { cx: "17", cy: "15", r: "3" }
                    path { d: "m21 19-1.9-1.9" }
                }
            }
        }
        Icon::CommandPalette => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" }
                }
            }
        }
        Icon::QuickSwitcher => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z" }
                }
            }
        }
        Icon::Shortcuts => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M10 8h.01" }
                    path { d: "M12 12h.01" }
                    path { d: "M14 8h.01" }
                    path { d: "M16 12h.01" }
                    path { d: "M18 8h.01" }
                    path { d: "M6 8h.01" }
                    path { d: "M7 16h10" }
                    path { d: "M8 12h.01" }
                    rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
                }
            }
        }
        Icon::DailyNote => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M8 2v4" }
                    path { d: "M16 2v4" }
                    rect { width: "18", height: "18", x: "3", y: "4", rx: "2" }
                    path { d: "M3 10h18" }
                }
            }
        }
        Icon::NewNote => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M5 12h14" }
                    path { d: "M12 5v14" }
                }
            }
        }
        Icon::ToggleSidebar => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M9 3v18" }
                }
            }
        }
        Icon::ToggleInspector => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "8", height: "4", x: "8", y: "2", rx: "1", ry: "1" }
                    path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
                    path { d: "M12 11h4" }
                    path { d: "M12 16h4" }
                    path { d: "M8 11h.01" }
                    path { d: "M8 16h.01" }
                }
            }
        }
        Icon::Home => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 21v-8a1 1 0 0 0-1-1h-4a1 1 0 0 0-1 1v8" }
                    path { d: "M3 10a2 2 0 0 1 .709-1.528l7-5.999a2 2 0 0 1 2.582 0l7 5.999A2 2 0 0 1 21 10v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" }
                }
            }
        }
        Icon::Back => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m12 19-7-7 7-7" }
                    path { d: "M19 12H5" }
                }
            }
        }
        Icon::Forward => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M5 12h14" }
                    path { d: "m12 5 7 7-7 7" }
                }
            }
        }
        Icon::ChevronDown => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m6 9 6 6 6-6" }
                }
            }
        }
        Icon::ChevronLeft => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m15 18-6-6 6-6" }
                }
            }
        }
        Icon::ChevronRight => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m9 18 6-6-6-6" }
                }
            }
        }
        Icon::ChevronUp => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m18 15-6-6-6 6" }
                }
            }
        }
        Icon::ChevronsLeft => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m11 17-5-5 5-5" }
                    path { d: "m18 17-5-5 5-5" }
                }
            }
        }
        Icon::ChevronsRight => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m6 17 5-5-5-5" }
                    path { d: "m13 17 5-5-5-5" }
                }
            }
        }
        Icon::Menu => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    line { x1: "4", x2: "20", y1: "12", y2: "12" }
                    line { x1: "4", x2: "20", y1: "6", y2: "6" }
                    line { x1: "4", x2: "20", y1: "18", y2: "18" }
                }
            }
        }
        Icon::MoreHorizontal => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "1" }
                    circle { cx: "19", cy: "12", r: "1" }
                    circle { cx: "5", cy: "12", r: "1" }
                }
            }
        }
        Icon::MoreVertical => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "1" }
                    circle { cx: "12", cy: "5", r: "1" }
                    circle { cx: "12", cy: "19", r: "1" }
                }
            }
        }
        Icon::PanelLeft => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M9 3v18" }
                }
            }
        }
        Icon::PanelRight => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M15 3v18" }
                }
            }
        }
        Icon::Folder => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" }
                }
            }
        }
        Icon::FolderOpen => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" }
                }
            }
        }
        Icon::FolderTree => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 10a1 1 0 0 0 1-1V6a1 1 0 0 0-1-1h-2.5a1 1 0 0 1-.8-.4l-.9-1.2A1 1 0 0 0 15 3h-2a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z" }
                    path { d: "M20 21a1 1 0 0 0 1-1v-3a1 1 0 0 0-1-1h-2.9a1 1 0 0 1-.88-.55l-.42-.85a1 1 0 0 0-.92-.6H13a1 1 0 0 0-1 1v5a1 1 0 0 0 1 1Z" }
                    path { d: "M3 5a2 2 0 0 0 2 2h3" }
                    path { d: "M3 3v13a2 2 0 0 0 2 2h3" }
                }
            }
        }
        Icon::File => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }
                    path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
                }
            }
        }
        Icon::FileText => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }
                    path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
                    path { d: "M10 9H8" }
                    path { d: "M16 13H8" }
                    path { d: "M16 17H8" }
                }
            }
        }
        Icon::FilePen => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12.5 22H18a2 2 0 0 0 2-2V7l-5-5H6a2 2 0 0 0-2 2v9.5" }
                    path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
                    path { d: "M13.378 15.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z" }
                }
            }
        }
        Icon::FilePlus => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" }
                    path { d: "M14 2v4a2 2 0 0 0 2 2h4" }
                    path { d: "M9 15h6" }
                    path { d: "M12 18v-6" }
                }
            }
        }
        Icon::StickyNote => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M16 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V8Z" }
                    path { d: "M15 3v4a2 2 0 0 0 2 2h4" }
                }
            }
        }
        Icon::NotebookText => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M2 6h4" }
                    path { d: "M2 10h4" }
                    path { d: "M2 14h4" }
                    path { d: "M2 18h4" }
                    rect { width: "16", height: "20", x: "4", y: "2", rx: "2" }
                    path { d: "M9.5 8h5" }
                    path { d: "M9.5 12H16" }
                    path { d: "M9.5 16H14" }
                }
            }
        }
        Icon::NotebookPen => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M13.4 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-7.4" }
                    path { d: "M2 6h4" }
                    path { d: "M2 10h4" }
                    path { d: "M2 14h4" }
                    path { d: "M2 18h4" }
                    path { d: "M21.378 5.626a1 1 0 1 0-3.004-3.004l-5.01 5.012a2 2 0 0 0-.506.854l-.837 2.87a.5.5 0 0 0 .62.62l2.87-.837a2 2 0 0 0 .854-.506z" }
                }
            }
        }
        Icon::BookCheck => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20" }
                    path { d: "m9 9.5 2 2 4-4" }
                }
            }
        }
        Icon::BookOpen => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 7v14" }
                    path { d: "M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z" }
                }
            }
        }
        Icon::BookText => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20" }
                    path { d: "M8 11h8" }
                    path { d: "M8 7h6" }
                }
            }
        }
        Icon::BookMarked => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M10 2v8l3-3 3 3V2" }
                    path { d: "M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20" }
                }
            }
        }
        Icon::Save => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15.2 3a2 2 0 0 1 1.4.6l3.8 3.8a2 2 0 0 1 .6 1.4V19a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2z" }
                    path { d: "M17 21v-7a1 1 0 0 0-1-1H8a1 1 0 0 0-1 1v7" }
                    path { d: "M7 3v4a1 1 0 0 0 1 1h7" }
                }
            }
        }
        Icon::Copy => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "14", height: "14", x: "8", y: "8", rx: "2", ry: "2" }
                    path { d: "M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" }
                }
            }
        }
        Icon::Clipboard => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "8", height: "4", x: "8", y: "2", rx: "1", ry: "1" }
                    path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
                }
            }
        }
        Icon::ClipboardList => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "8", height: "4", x: "8", y: "2", rx: "1", ry: "1" }
                    path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
                    path { d: "M12 11h4" }
                    path { d: "M12 16h4" }
                    path { d: "M8 11h.01" }
                    path { d: "M8 16h.01" }
                }
            }
        }
        Icon::Tag => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z" }
                    circle { cx: "7.5", cy: "7.5", r: ".5", fill: "currentColor" }
                }
            }
        }
        Icon::PenLine => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 20h9" }
                    path { d: "M16.376 3.622a1 1 0 0 1 3.002 3.002L7.368 18.635a2 2 0 0 1-.855.506l-2.872.838a.5.5 0 0 1-.62-.62l.838-2.872a2 2 0 0 1 .506-.854z" }
                }
            }
        }
        Icon::Image => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2", ry: "2" }
                    circle { cx: "9", cy: "9", r: "2" }
                    path { d: "m21 15-3.086-3.086a2 2 0 0 0-2.828 0L6 21" }
                }
            }
        }
        Icon::ToastSuccess => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m9 12 2 2 4-4" }
                }
            }
        }
        Icon::ToastWarning => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" }
                    path { d: "M12 9v4" }
                    path { d: "M12 17h.01" }
                }
            }
        }
        Icon::ToastError => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m15 9-6 6" }
                    path { d: "m9 9 6 6" }
                }
            }
        }
        Icon::ToastInfo => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "M12 16v-4" }
                    path { d: "M12 8h.01" }
                }
            }
        }
        Icon::Bell => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" }
                    path { d: "M10.3 21a1.94 1.94 0 0 0 3.4 0" }
                }
            }
        }
        Icon::BellRing => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" }
                    path { d: "M10.3 21a1.94 1.94 0 0 0 3.4 0" }
                    path { d: "M4 2C2.8 3.7 2 5.7 2 8" }
                    path { d: "M22 8c0-2.3-.8-4.3-2-6" }
                }
            }
        }
        Icon::Warning => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" }
                    path { d: "M12 9v4" }
                    path { d: "M12 17h.01" }
                }
            }
        }
        Icon::CircleAlert => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    line { x1: "12", x2: "12", y1: "8", y2: "12" }
                    line { x1: "12", x2: "12.01", y1: "16", y2: "16" }
                }
            }
        }
        Icon::Circle => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                }
            }
        }
        Icon::Info => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "M12 16v-4" }
                    path { d: "M12 8h.01" }
                }
            }
        }
        Icon::Check => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 6 9 17l-5-5" }
                }
            }
        }
        Icon::CircleCheck => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m9 12 2 2 4-4" }
                }
            }
        }
        Icon::CircleX => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m15 9-6 6" }
                    path { d: "m9 9 6 6" }
                }
            }
        }
        Icon::X => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M18 6 6 18" }
                    path { d: "m6 6 12 12" }
                }
            }
        }
        Icon::Plus => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M5 12h14" }
                    path { d: "M12 5v14" }
                }
            }
        }
        Icon::Loader => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 2v4" }
                    path { d: "m16.2 7.8 2.9-2.9" }
                    path { d: "M18 12h4" }
                    path { d: "m16.2 16.2 2.9 2.9" }
                    path { d: "M12 18v4" }
                    path { d: "m4.9 19.1 2.9-2.9" }
                    path { d: "M2 12h4" }
                    path { d: "m4.9 4.9 2.9 2.9" }
                }
            }
        }
        Icon::RefreshCw => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" }
                    path { d: "M21 3v5h-5" }
                    path { d: "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" }
                    path { d: "M8 16H3v5" }
                }
            }
        }
        Icon::RefreshCcw => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                    path { d: "M3 3v5h5" }
                    path { d: "M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" }
                    path { d: "M16 16h5v5" }
                }
            }
        }
        Icon::LifeBuoy => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "m4.93 4.93 4.24 4.24" }
                    path { d: "m14.83 9.17 4.24-4.24" }
                    path { d: "m14.83 14.83 4.24 4.24" }
                    path { d: "m9.17 14.83-4.24 4.24" }
                    circle { cx: "12", cy: "12", r: "4" }
                }
            }
        }
        Icon::Star => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z" }
                }
            }
        }
        Icon::StarHalf => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 18.338a2.1 2.1 0 0 0-.987.244L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.12 2.12 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.12 2.12 0 0 0 1.597-1.16l2.309-4.679A.53.53 0 0 1 12 2" }
                }
            }
        }
        Icon::Pin => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0" }
                    circle { cx: "12", cy: "10", r: "3" }
                }
            }
        }
        Icon::Edit => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 20h9" }
                    path { d: "M16.376 3.622a1 1 0 0 1 3.002 3.002L7.368 18.635a2 2 0 0 1-.855.506l-2.872.838a.5.5 0 0 1-.62-.62l.838-2.872a2 2 0 0 1 .506-.854z" }
                }
            }
        }
        Icon::Delete => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 6h18" }
                    path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
                    path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
                    line { x1: "10", x2: "10", y1: "11", y2: "17" }
                    line { x1: "14", x2: "14", y1: "11", y2: "17" }
                }
            }
        }
        Icon::Undo => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 7v6h6" }
                    path { d: "M21 17a9 9 0 0 0-9-9 9 9 0 0 0-6 2.3L3 13" }
                }
            }
        }
        Icon::Redo => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 7v6h-6" }
                    path { d: "M3 17a9 9 0 0 1 9-9 9 9 0 0 1 6 2.3l3 2.7" }
                }
            }
        }
        Icon::Mail => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
                    path { d: "m22 7-8.97 5.7a1.94 1.94 0 0 1-2.06 0L2 7" }
                }
            }
        }
        Icon::Send => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z" }
                    path { d: "m21.854 2.147-10.94 10.939" }
                }
            }
        }
        Icon::Share => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 12v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-8" }
                    polyline { points: "16 6 12 2 8 6" }
                    line { x1: "12", x2: "12", y1: "2", y2: "15" }
                }
            }
        }
        Icon::Share2 => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "18", cy: "5", r: "3" }
                    circle { cx: "6", cy: "12", r: "3" }
                    circle { cx: "18", cy: "19", r: "3" }
                    line { x1: "8.59", x2: "15.42", y1: "13.51", y2: "17.49" }
                    line { x1: "15.41", x2: "8.59", y1: "6.51", y2: "10.49" }
                }
            }
        }
        Icon::Reply => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "9 17 4 12 9 7" }
                    path { d: "M20 18v-2a4 4 0 0 0-4-4H4" }
                }
            }
        }
        Icon::ReplyAll => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "7 17 2 12 7 7" }
                    polyline { points: "12 17 7 12 12 7" }
                    path { d: "M22 18v-2a4 4 0 0 0-4-4H7" }
                }
            }
        }
        Icon::Network => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { x: "16", y: "16", width: "6", height: "6", rx: "1" }
                    rect { x: "2", y: "16", width: "6", height: "6", rx: "1" }
                    rect { x: "9", y: "2", width: "6", height: "6", rx: "1" }
                    path { d: "M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3" }
                    path { d: "M12 12V8" }
                }
            }
        }
        Icon::Link => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" }
                    path { d: "M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" }
                }
            }
        }
        Icon::Link2 => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M9 17H7A5 5 0 0 1 7 7h2" }
                    path { d: "M15 7h2a5 5 0 1 1 0 10h-2" }
                    line { x1: "8", x2: "16", y1: "12", y2: "12" }
                }
            }
        }
        Icon::MessageCircle => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M7.9 20A9 9 0 1 0 4 16.1L2 22Z" }
                }
            }
        }
        Icon::MessageSquare => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
                }
            }
        }
        Icon::Comparison => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "18", cy: "18", r: "3" }
                    circle { cx: "6", cy: "6", r: "3" }
                    path { d: "M13 6h3a2 2 0 0 1 2 2v7" }
                    path { d: "M11 18H8a2 2 0 0 1-2-2V9" }
                }
            }
        }
        Icon::Clock => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    polyline { points: "12 6 12 12 16 14" }
                }
            }
        }
        Icon::Timer => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    polyline { points: "12 6 12 12 16 14" }
                }
            }
        }
        Icon::List => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 12h.01" }
                    path { d: "M3 18h.01" }
                    path { d: "M3 6h.01" }
                    path { d: "M8 12h13" }
                    path { d: "M8 18h13" }
                    path { d: "M8 6h13" }
                }
            }
        }
        Icon::ListChecks => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m3 17 2 2 4-4" }
                    path { d: "m3 7 2 2 4-4" }
                    path { d: "M13 6h8" }
                    path { d: "M13 12h8" }
                    path { d: "M13 18h8" }
                }
            }
        }
        Icon::Grid => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M3 12h18" }
                    path { d: "M12 3v18" }
                }
            }
        }
        Icon::Columns => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M12 3v18" }
                }
            }
        }
        Icon::Table => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 3v18" }
                    rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
                    path { d: "M3 9h18" }
                    path { d: "M3 15h18" }
                }
            }
        }
        Icon::Kanban => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M6 5v11" }
                    path { d: "M12 5v6" }
                    path { d: "M18 5v14" }
                }
            }
        }
        Icon::Gallery => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M7 2h10" }
                    path { d: "M5 6h14" }
                    rect { width: "18", height: "12", x: "3", y: "10", rx: "2" }
                }
            }
        }
        Icon::GalleryVerticalEnd => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M7 2h10" }
                    path { d: "M5 6h14" }
                    rect { width: "18", height: "12", x: "3", y: "10", rx: "2" }
                }
            }
        }
        Icon::Palette => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "13.5", cy: "6.5", r: ".5", fill: "currentColor" }
                    circle { cx: "17.5", cy: "10.5", r: ".5", fill: "currentColor" }
                    circle { cx: "8.5", cy: "7.5", r: ".5", fill: "currentColor" }
                    circle { cx: "6.5", cy: "12.5", r: ".5", fill: "currentColor" }
                    path { d: "M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.554C21.965 6.012 17.461 2 12 2z" }
                }
            }
        }
        Icon::Layers => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m12.83 2.18a2 2 0 0 0-1.66 0L2.6 6.08a1 1 0 0 0 0 1.83l8.58 3.91a2 2 0 0 0 1.66 0l8.58-3.9a1 1 0 0 0 0-1.83Z" }
                    path { d: "m22 17.65-9.17 4.16a2 2 0 0 1-1.66 0L2 17.65" }
                    path { d: "m22 12.65-9.17 4.16a2 2 0 0 1-1.66 0L2 12.65" }
                }
            }
        }
        Icon::Library => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m16 6 4 14" }
                    path { d: "M12 6v14" }
                    path { d: "M8 8v12" }
                    path { d: "M4 4v16" }
                }
            }
        }
        Icon::Brush => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m9.06 11.9 8.07-8.06a2.85 2.85 0 1 1 4.03 4.03l-8.06 8.08" }
                    path { d: "M7.07 14.94c-1.66 0-3 1.35-3 3.02 0 1.33-2.5 1.52-2 2.02 1.08 1.1 2.49 2.02 4 2.02 2.2 0 4-1.8 4-4.04a3.01 3.01 0 0 0-3-3.02z" }
                }
            }
        }
        Icon::Mic => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 2a3 3 0 0 0-3 3v7a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3Z" }
                    path { d: "M19 10v2a7 7 0 0 1-14 0v-2" }
                    line { x1: "12", x2: "12", y1: "19", y2: "22" }
                }
            }
        }
        Icon::Camera => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" }
                    circle { cx: "12", cy: "13", r: "3" }
                }
            }
        }
        Icon::Music => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M9 18V5l12-2v13" }
                    circle { cx: "6", cy: "18", r: "3" }
                    circle { cx: "18", cy: "16", r: "3" }
                }
            }
        }
        Icon::Play => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polygon { points: "6 3 20 12 6 21 6 3" }
                }
            }
        }
        Icon::Package => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M11 21.73a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73z" }
                    path { d: "M12 22V12" }
                    path { d: "m3.3 7 7.703 4.734a2 2 0 0 0 1.994 0L20.7 7" }
                    path { d: "m7.5 4.27 9 5.15" }
                }
            }
        }
        Icon::Target => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    circle { cx: "12", cy: "12", r: "6" }
                    circle { cx: "12", cy: "12", r: "2" }
                }
            }
        }
        Icon::Command => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 6v12a3 3 0 1 0 3-3H6a3 3 0 1 0 3 3V6a3 3 0 1 0-3 3h12a3 3 0 1 0-3-3" }
                }
            }
        }
        Icon::Monitor => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "20", height: "14", x: "2", y: "3", rx: "2" }
                    line { x1: "8", x2: "16", y1: "21", y2: "21" }
                    line { x1: "12", x2: "12", y1: "17", y2: "21" }
                }
            }
        }
        Icon::Smartphone => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "14", height: "20", x: "5", y: "2", rx: "2", ry: "2" }
                    path { d: "M12 18h.01" }
                }
            }
        }
        Icon::Tablet => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    rect { width: "16", height: "20", x: "4", y: "2", rx: "2", ry: "2" }
                    line { x1: "12", x2: "12.01", y1: "18", y2: "18" }
                }
            }
        }
        Icon::Laptop => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 16V7a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v9m16 0H4m16 0 1.28 2.55a1 1 0 0 1-.9 1.45H3.62a1 1 0 0 1-.9-1.45L4 16" }
                }
            }
        }
        Icon::User => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }
                    circle { cx: "12", cy: "7", r: "4" }
                }
            }
        }
        Icon::MapPin => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0" }
                    circle { cx: "12", cy: "10", r: "3" }
                }
            }
        }
        Icon::HardDrive => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    line { x1: "22", x2: "2", y1: "12", y2: "12" }
                    path { d: "M5.45 5.11 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.89A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.11z" }
                    line { x1: "6", x2: "6.01", y1: "16", y2: "16" }
                    line { x1: "10", x2: "10.01", y1: "16", y2: "16" }
                }
            }
        }
        Icon::ChartBar => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 3v16a2 2 0 0 0 2 2h16" }
                    path { d: "M7 16h8" }
                    path { d: "M7 11h12" }
                    path { d: "M7 6h3" }
                }
            }
        }
        Icon::ChartColumn => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 3v16a2 2 0 0 0 2 2h16" }
                    path { d: "M18 17V9" }
                    path { d: "M13 17V5" }
                    path { d: "M8 17v-3" }
                }
            }
        }
        Icon::ChartLine => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 3v16a2 2 0 0 0 2 2h16" }
                    path { d: "m19 9-5 5-4-4-3 3" }
                }
            }
        }
        Icon::ChartPie => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 12c.552 0 1.005-.449.95-.998a10 10 0 0 0-8.953-8.951c-.55-.055-.998.398-.998.95v8a1 1 0 0 0 1 1z" }
                    path { d: "M21.21 15.89A10 10 0 1 1 8 2.83" }
                }
            }
        }
        Icon::TrendingUp => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "22 7 13.5 15.5 8.5 10.5 2 17" }
                    polyline { points: "16 7 22 7 22 13" }
                }
            }
        }
        Icon::TrendingDown => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "22 17 13.5 8.5 8.5 13.5 2 7" }
                    polyline { points: "16 17 22 17 22 11" }
                }
            }
        }
        Icon::Flame => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z" }
                }
            }
        }
        Icon::KanbanBoard => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M6 5v11" }
                    path { d: "M12 5v6" }
                    path { d: "M18 5v14" }
                }
            }
        }
        Icon::VisionOcr => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M3 7V5a2 2 0 0 1 2-2h2" }
                    path { d: "M17 3h2a2 2 0 0 1 2 2v2" }
                    path { d: "M21 17v2a2 2 0 0 1-2 2h-2" }
                    path { d: "M7 21H5a2 2 0 0 1-2-2v-2" }
                    path { d: "M7 8h8" }
                    path { d: "M7 12h10" }
                    path { d: "M7 16h6" }
                }
            }
        }
        Icon::CodeBlock => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    polyline { points: "16 18 22 12 16 6" }
                    polyline { points: "8 6 2 12 8 18" }
                }
            }
        }
        Icon::Callout => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 14c.2-1 .7-1.7 1.5-2.5 1-.9 1.5-2.2 1.5-3.5A6 6 0 0 0 6 8c0 1 .2 2.2 1.5 3.5.7.7 1.3 1.5 1.5 2.5" }
                    path { d: "M9 18h6" }
                    path { d: "M10 22h4" }
                }
            }
        }
        Icon::Sparkles => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z" }
                    path { d: "M20 3v4" }
                    path { d: "M22 5h-4" }
                    path { d: "M4 17v2" }
                    path { d: "M5 18H3" }
                }
            }
        }
        Icon::Zap => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M4 14a1 1 0 0 1-.78-1.63l9.9-10.2a.5.5 0 0 1 .86.46l-1.92 6.02A1 1 0 0 0 13 10h7a1 1 0 0 1 .78 1.63l-9.9 10.2a.5.5 0 0 1-.86-.46l1.92-6.02A1 1 0 0 0 11 14z" }
                }
            }
        }
        Icon::Wand => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 4V2" }
                    path { d: "M15 16v-2" }
                    path { d: "M8 9h2" }
                    path { d: "M20 9h2" }
                    path { d: "M17.8 11.8 19 13" }
                    path { d: "M15 9h.01" }
                    path { d: "M17.8 6.2 19 5" }
                    path { d: "m3 21 9-9" }
                    path { d: "M12.2 6.2 11 5" }
                }
            }
        }
        Icon::Globe => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" }
                    path { d: "M2 12h20" }
                }
            }
        }
        Icon::CircleHelp => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" }
                    path { d: "M12 17h.01" }
                }
            }
        }
        Icon::Bookmark => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" }
                }
            }
        }
        Icon::CloudUpload => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 13v8" }
                    path { d: "M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" }
                    path { d: "m8 17 4-4 4 4" }
                }
            }
        }
        Icon::Upload => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                    polyline { points: "17 8 12 3 7 8" }
                    line { x1: "12", x2: "12", y1: "3", y2: "15" }
                }
            }
        }
        Icon::Download => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
                    polyline { points: "7 10 12 15 17 10" }
                    line { x1: "12", x2: "12", y1: "15", y2: "3" }
                }
            }
        }
        Icon::Scissors => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "6", cy: "6", r: "3" }
                    path { d: "M8.12 8.12 12 12" }
                    path { d: "M20 4 8.12 15.88" }
                    circle { cx: "6", cy: "18", r: "3" }
                    path { d: "M14.8 14.8 20 20" }
                }
            }
        }
        Icon::ExternalLink => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M15 3h6v6" }
                    path { d: "M10 14 21 3" }
                    path { d: "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6" }
                }
            }
        }
        Icon::Sun => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "4" }
                    path { d: "M12 2v2" }
                    path { d: "M12 20v2" }
                    path { d: "m4.93 4.93 1.41 1.41" }
                    path { d: "m17.66 17.66 1.41 1.41" }
                    path { d: "M2 12h2" }
                    path { d: "M20 12h2" }
                    path { d: "m6.34 17.66-1.41 1.41" }
                    path { d: "m19.07 4.93-1.41 1.41" }
                }
            }
        }
        Icon::Moon => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" }
                }
            }
        }
        Icon::Ellipsis => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "1" }
                    circle { cx: "19", cy: "12", r: "1" }
                    circle { cx: "5", cy: "12", r: "1" }
                }
            }
        }
        Icon::CircleEllipsis => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "12", cy: "12", r: "10" }
                    path { d: "M17 12h.01" }
                    path { d: "M12 12h.01" }
                    path { d: "M7 12h.01" }
                }
            }
        }
        Icon::Eye => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" }
                    circle { cx: "12", cy: "12", r: "3" }
                }
            }
        }
        Icon::EyeOff => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" }
                    path { d: "M14.084 14.158a3 3 0 0 1-4.242-4.242" }
                    path { d: "M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" }
                    path { d: "m2 2 20 20" }
                }
            }
        }
        Icon::Keyboard => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M10 8h.01" }
                    path { d: "M12 12h.01" }
                    path { d: "M14 8h.01" }
                    path { d: "M16 12h.01" }
                    path { d: "M18 8h.01" }
                    path { d: "M6 8h.01" }
                    path { d: "M7 16h10" }
                    path { d: "M8 12h.01" }
                    rect { width: "20", height: "16", x: "2", y: "4", rx: "2" }
                }
            }
        }
        Icon::GitCompare => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    circle { cx: "18", cy: "18", r: "3" }
                    circle { cx: "6", cy: "6", r: "3" }
                    path { d: "M13 6h3a2 2 0 0 1 2 2v7" }
                    path { d: "M11 18H8a2 2 0 0 1-2-2V9" }
                }
            }
        }
        Icon::Activity => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M2 12a10 10 0 0 1 19.42-2.34" }
                    path { d: "M2 12a10 10 0 0 0 19.42 2.34" }
                    path { d: "M2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6" }
                    path { d: "M8 8l4-4 4 4" }
                    path { d: "M8 16l4 4 4-4" }
                }
            }
        }
        Icon::Slash => {
            rsx! {
                svg {
                    xmlns: "http://www.w3.org/2000/svg",
                    width: "24",
                    height: "24",
                    view_box: "0 0 24 24",
                    fill: "none",
                    stroke: "currentColor",
                    stroke_width: "2",
                    stroke_linecap: "round",
                    stroke_linejoin: "round",
                    class: "lucide",
                    path { d: "M22 2 2 22" }
                }
            }
        }
    }
}

/// Plain function form of the icon renderer — returns a themed SVG view
/// directly (no wrapping span), for use inside other `rsx!` blocks as
/// `{render_icon_view(icon)}`.
pub fn render_icon_view(icon: Icon) -> Element {
    icon_component(icon)
}

/// Render an [`Icon`] inside an optional wrapping `<span>` so callers can drop
/// it in place of a text emoji node inside an `rsx!` block:
/// `{render_icon(Icon::Search, Some("text-2xl"))}`.
///
/// The wrapping span carries `aria-hidden="true"` so decorative icons are
/// excluded from the screen-reader tree. Icon-only controls that use this
/// helper already carry an `aria-label` on their host element.
///
/// The SVG scales with the parent's `font-size` (via the global `.lucide`
/// CSS rule) so existing `text-*` / `text-[...]` sizing classes still control
/// the icon footprint, exactly like the emoji glyphs they replace.
pub fn render_icon(icon: Icon, class: Option<&'static str>) -> Element {
    match class {
        Some(extra) => rsx! {
            span {
                class: format!("lucide-icon {}", extra),
                "aria-hidden": "true",
                {render_icon_view(icon)}
            }
        },
        None => rsx! {
            span {
                class: "lucide-icon",
                "aria-hidden": "true",
                {render_icon_view(icon)}
            }
        },
    }
}

/// Convenience wrapper: render an [`Icon`] inside a sized, theme-coloured span.
/// Mirrors the previous emoji pattern (`<span class="...">{emoji}</span>`).
///
/// When `label` is `None` the icon is decorative (`aria-hidden="true"`); when
/// `Some` the icon carries meaning and the label is exposed to assistive
/// technology via a `title`.
#[component]
pub fn IconEl(
    /// Which icon to draw.
    icon: Icon,
    /// Optional extra Tailwind/utility classes (e.g. sizing).
    #[props(optional)]
    class: Option<&'static str>,
    /// Optional accessible label. When `None` the icon is decorative.
    #[props(optional)]
    label: Option<&'static str>,
) -> Element {
    let aria = label.is_none();
    let cls = format!("lucide-icon {}", class.unwrap_or(""));
    rsx! {
        span {
            class: cls,
            "aria-hidden": aria,
            title: label,
            {icon_component(icon)}
        }
    }
}
