//! # Nabu Shared Component Library
//!
//! Reusable, accessible, theme-aware UI primitives built on the design system
//! in `src/styles/app.css`. Every future screen should compose these
//! components instead of hand-rolling one-off markup.
//!
//! Modules:
//! - [`button`] — Button with 6 variants, sizes, loading & icon modes
//! - [`input`] — text, textarea, search, password, number inputs + validation
//! - [`selection`] — checkbox, radio, switch, segmented control, select
//! - [`card`] — standard / outlined / elevated / interactive / collapsible
//! - [`dialog`] — modal, confirm, alert and prompt dialogs
//! - [`menu`] — dropdown, context, command and overflow menus
//! - [`nav`] — tabs, breadcrumbs, sidebar items, toolbar buttons, nav groups
//! - [`feedback`] — toasts, banners, alerts, badges, progress, spinners, skeletons
//! - [`info`] — tooltips, empty states, callouts, help text
//! - [`layout`] — panels, sections, stacks, grids, containers

pub mod button;
pub mod card;
pub mod dialog;
pub mod feedback;
pub mod info;
pub mod input;
pub mod layout;
pub mod menu;
pub mod nav;
pub mod selection;

pub use button::{Button, ButtonSize, ButtonVariant, IconButton};
pub use card::{Card, CardBody, CardFooter, CardHeader, CardVariant, CollapsibleCard};
pub use dialog::{AlertDialog, ConfirmDialog, Dialog, DialogSize, PromptDialog};
pub use feedback::{
    Alert, Badge, BadgeKind, Banner, Progress, Skeleton, Spinner, SpinnerSize, StatusDot,
    ToastKind, ToastProvider, ToastRegion, use_toast,
};
pub use info::{Callout, CalloutKind, EmptyState, HelpText, Tooltip};
pub use input::{NumberInput, PasswordInput, SearchInput, TextInput, Textarea};
pub use layout::{Container, ContainerWidth, Grid, Panel, Section, Stack, StackDirection, StackGap};
pub use menu::{
    CommandItem, CommandMenu, ContextMenu, DropdownMenu, MenuItem, MenuSeparator, OverflowMenu,
};
pub use nav::{
    Breadcrumb, Breadcrumbs, NavGroup, SidebarItem, TabDef, Tabs, ToolbarButton,
};
pub use selection::{Checkbox, Radio, Segmented, SegmentedOption, Select, SelectOption, Switch};
