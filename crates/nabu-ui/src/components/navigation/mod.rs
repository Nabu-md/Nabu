//! # Navigation & Knowledge Discovery
//!
//! Phase 12.2 surfaces Nabu's features through four navigational surfaces:
//!
//! - **Dashboard** — a configurable home (recently modified, favourites,
//!   recently opened, pinned tabs, inbox, recent searches, summary)
//! - **Home screen** — shown when no note is selected (welcome, quick actions,
//!   recent activity)
//! - **Search page** — full-text search with snippets, highlighted matches,
//!   filters, sorting, recent + saved searches
//! - **Command Palette / Quick Switcher / Shortcuts reference** — keyboard-first
//!   discovery of every command and note
//!
//! Shared state lives in [`state`] (the [`NavContext`](state::NavContext)),
//! persisted to the backend settings store under dedicated keys.

pub mod archive_page;
pub mod breadcrumb;
pub mod calendar_page;
pub mod command_palette;
pub mod commands;
pub mod dashboard;
pub mod home_screen;
pub mod navbar;
pub mod quick_switcher;
pub mod search_page;
pub mod shortcuts;
pub mod smart_folders;
pub mod state;

pub use archive_page::ArchivePage;
pub use breadcrumb::BreadcrumbBar;
pub use calendar_page::CalendarPage;
pub use command_palette::CommandPalette;
pub use dashboard::Dashboard;
pub use home_screen::HomeScreen;
pub use navbar::NavBar;
pub use quick_switcher::QuickSwitcher;
pub use search_page::SearchPage;
pub use shortcuts::ShortcutReference;
pub use smart_folders::SmartFoldersPage;
pub use state::{load_all_nav_state, NavContext, provide_navigation, use_nav};
