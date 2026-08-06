//! Layout modules — structural scaffolding for the workspace.

pub mod left_sidebar;
pub mod ribbon_bar;
pub mod right_inspector;
pub mod tab_bar;
pub mod workspace;

pub use left_sidebar::LeftSidebar;
pub use ribbon_bar::RibbonBar;
pub use right_inspector::RightInspector;
pub use tab_bar::TabBar;
pub use workspace::WorkspaceLayout;
