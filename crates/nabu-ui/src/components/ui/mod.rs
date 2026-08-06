//! # Nabu Shared Component Library
//!
//! Re-exports the icon system and Dioxus-native UI primitives.

pub mod icons;
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

pub use icons::{render_icon, render_icon_view, Icon, IconEl};
pub use button::{Button, ButtonVariant, ButtonSize, IconButton, button_classes};
pub use card::{Card, CardVariant, CardHeader, CardBody, CardFooter, CollapsibleCard};
pub use dialog::{Dialog, DialogSize, ConfirmDialog, AlertDialog, PromptDialog};
pub use feedback::{
    ToastKind, ToastItem, ToastAction, ToastContext, ToastProvider,
    use_toast, Banner, Alert, Badge, BadgeKind, Progress,
    Spinner, SpinnerSize, Skeleton, StatusDot, StatusKind,
    LoadingBlock, LoadingOverlay, LoadingScreen, SkeletonList,
    ErrorPanel, TaskInfo, TaskContext, provide_tasks, use_tasks,
    TaskIndicator, set_timeout,
};
pub use info::{Tooltip, EmptyState, Callout, CalloutKind, HelpText};
pub use input::{TextInput, Textarea, SearchInput, PasswordInput, NumberInput};
pub use layout::{Panel, Section, Stack, StackDirection, StackGap, Grid, Container, ContainerWidth};
pub use menu::{MenuItem, MenuSeparator, DropdownMenu, OverflowMenu, CommandItem, CommandMenu, ContextMenu};
pub use nav::{TabDef, Tabs, Breadcrumb, Breadcrumbs, SidebarItem, ToolbarButton, NavGroup};
pub use selection::{
    Checkbox, Radio, Switch, Segmented, SegmentedOption, Select, SelectOption,
};
