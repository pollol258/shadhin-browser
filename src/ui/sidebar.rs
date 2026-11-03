//! Translucent burgundy sidebar with navigation items and workspaces
//!
//! Provides the primary navigation rail containing Home, Bookmarks, History,
//! Downloads, open tabs list, Workspaces, and pinned Settings.
//! Supports Expanded full rail (~240px) and Compact icon rail (~56px).

use crate::ui::tabbar::TabDisplayInfo;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ShadhinSidebar {
    container: gtk4::Box,
    workspaces_box: gtk4::Box,
    on_navigate_page: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    on_workspace_selected: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    on_create_workspace: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    tabs_list_box: gtk4::Box,
    tabs_expander: gtk4::Expander,
    is_compact: Rc<RefCell<bool>>,
}

impl ShadhinSidebar {
    /// Constructs a new sidebar component
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .css_classes(["shadhin-sidebar"])
            .build();

        let is_compact = Rc::new(RefCell::new(false));
        let on_navigate_page: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> = Rc::new(RefCell::new(None));
        let on_workspace_selected: Rc<RefCell<Option<Box<dyn Fn(&str)>>>> =
            Rc::new(RefCell::new(None));
        let on_create_workspace: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));

        // 1. Primary Navigation Items
        let (home_btn, _) = Self::create_sidebar_item("go-home-symbolic", "Home", true);
        let on_nav_home = on_navigate_page.clone();
        home_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_nav_home.borrow() {
                cb("home");
            }
        });
        container.append(&home_btn);

        let nav_items = [
            (
                "user-bookmarks-symbolic",
                "Bookmarks",
                "bookmarks",
                "Ctrl+Shift+O",
            ),
            (
                "document-open-recent-symbolic",
                "History",
                "history",
                "Ctrl+H",
            ),
            (
                "folder-download-symbolic",
                "Downloads",
                "downloads",
                "Ctrl+J",
            ),
        ];

        for (icon, label, page_id, shortcut) in nav_items {
            let (btn, _) = Self::create_sidebar_item(icon, label, false);
            Self::setup_compact_overlay(
                &btn,
                label,
                page_id,
                shortcut,
                &is_compact,
                &on_navigate_page,
            );
            container.append(&btn);
        }

        // 2. Expandable Tabs Section
        let tabs_list_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(2)
            .margin_start(8)
            .build();

        let tabs_expander = gtk4::Expander::builder()
            .label("Open Tabs")
            .expanded(true)
            .child(&tabs_list_box)
            .css_classes(["shadhin-sidebar-item", "shadhin-tabs-expander"])
            .tooltip_text("Open Tabs")
            .build();
        container.append(&tabs_expander);

        // 3. Workspaces Section
        let sep = gtk4::Separator::new(gtk4::Orientation::Horizontal);
        sep.set_margin_top(8);
        sep.set_margin_bottom(4);
        sep.add_css_class("shadhin-sidebar-separator");
        container.append(&sep);

        let ws_title = gtk4::Label::builder()
            .label("Workspaces")
            .css_classes(["shadhin-sidebar-section-title"])
            .halign(gtk4::Align::Start)
            .build();
        container.append(&ws_title);

        let workspaces_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(2)
            .build();

        let workspaces = [
            ("personal", "Personal", "default"),
            ("work", "Work", "work"),
            ("project", "Project", "project"),
        ];

        for (dot_class, name, ws_id) in workspaces {
            let row = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .valign(gtk4::Align::Center)
                .css_classes(["shadhin-sidebar-item"])
                .build();

            let dot = gtk4::Box::builder()
                .css_classes(["shadhin-workspace-dot", dot_class])
                .valign(gtk4::Align::Center)
                .build();
            row.append(&dot);

            let lbl = gtk4::Label::builder()
                .label(name)
                .halign(gtk4::Align::Start)
                .hexpand(true)
                .css_classes(["shadhin-sidebar-label"])
                .build();
            row.append(&lbl);

            let btn = gtk4::Button::builder()
                .child(&row)
                .tooltip_text(name)
                .css_classes(["flat"])
                .build();

            let id_str = ws_id.to_string();
            let on_ws = on_workspace_selected.clone();
            btn.connect_clicked(move |_| {
                if let Some(ref cb) = *on_ws.borrow() {
                    cb(&id_str);
                }
            });
            workspaces_box.append(&btn);
        }
        container.append(&workspaces_box);

        // "+ New Workspace" button with icon for compact mode
        let new_ws_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk4::Align::Center)
            .build();
        let new_ws_icon = gtk4::Image::from_icon_name("list-add-symbolic");
        new_ws_icon.set_pixel_size(14);
        new_ws_box.append(&new_ws_icon);

        let new_ws_lbl = gtk4::Label::builder()
            .label("New Workspace")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .css_classes(["shadhin-sidebar-label"])
            .build();
        new_ws_box.append(&new_ws_lbl);

        let new_ws_btn = gtk4::Button::builder()
            .child(&new_ws_box)
            .tooltip_text("Create New Workspace")
            .css_classes(["flat", "shadhin-sidebar-item"])
            .halign(gtk4::Align::Fill)
            .build();
        let on_create = on_create_workspace.clone();
        new_ws_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_create.borrow() {
                cb();
            }
        });
        container.append(&new_ws_btn);

        // 4. Spacer to push Settings to the bottom
        let spacer = gtk4::Box::builder().vexpand(true).build();
        container.append(&spacer);

        // 5. Pinned Settings
        let (settings_btn, _) =
            Self::create_sidebar_item("preferences-system-symbolic", "Settings", false);
        Self::setup_compact_overlay(
            &settings_btn,
            "Settings",
            "settings",
            "Ctrl+,",
            &is_compact,
            &on_navigate_page,
        );
        container.append(&settings_btn);

        Self {
            container,
            workspaces_box,
            on_navigate_page,
            on_workspace_selected,
            on_create_workspace,
            tabs_list_box,
            tabs_expander,
            is_compact,
        }
    }

    /// Sets or clears compact mode (narrow ~56px icon-only rail vs full ~240px)
    pub fn set_compact_mode(&self, compact: bool) {
        *self.is_compact.borrow_mut() = compact;
        if compact {
            self.container.add_css_class("compact");
            self.container.set_width_request(56);
            self.tabs_expander.set_expanded(false);
        } else {
            self.container.remove_css_class("compact");
            self.container.set_width_request(240);
            self.tabs_expander.set_expanded(true);
        }
    }

    /// Returns whether the sidebar is currently in compact mode
    pub fn is_compact_mode(&self) -> bool {
        *self.is_compact.borrow()
    }

    /// Appends a new workspace entry to the sidebar
    pub fn add_workspace_item(&self, ws_id: &str, name: &str) {
        let row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk4::Align::Center)
            .css_classes(["shadhin-sidebar-item"])
            .build();

        let dot = gtk4::Box::builder()
            .css_classes(["shadhin-workspace-dot", "project"])
            .valign(gtk4::Align::Center)
            .build();
        row.append(&dot);

        let lbl = gtk4::Label::builder()
            .label(name)
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .css_classes(["shadhin-sidebar-label"])
            .build();
        row.append(&lbl);

        let btn = gtk4::Button::builder()
            .child(&row)
            .tooltip_text(name)
            .css_classes(["flat"])
            .build();

        let id_str = ws_id.to_string();
        let on_ws = self.on_workspace_selected.clone();
        btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_ws.borrow() {
                cb(&id_str);
            }
        });
        self.workspaces_box.append(&btn);
    }

    /// Registers callback for creating a new workspace
    pub fn set_on_create_workspace<F: Fn() + 'static>(&self, callback: F) {
        *self.on_create_workspace.borrow_mut() = Some(Box::new(callback));
    }

    /// Registers callback for workspace selection
    pub fn set_on_workspace_selected<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_workspace_selected.borrow_mut() = Some(Box::new(callback));
    }

    /// Helper to setup an overlay popover in compact mode, or direct navigation in expanded mode
    fn setup_compact_overlay(
        btn: &gtk4::Button,
        title: &str,
        page_id: &str,
        shortcut: &str,
        is_compact: &Rc<RefCell<bool>>,
        on_navigate: &Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    ) {
        let popover = gtk4::Popover::new();
        popover.set_parent(btn);
        popover.set_autohide(true);

        let pop_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .margin_start(10)
            .margin_end(10)
            .margin_top(10)
            .margin_bottom(10)
            .css_classes(["shadhin-sidebar-compact-popover"])
            .build();

        let title_lbl = gtk4::Label::builder()
            .label(title)
            .css_classes(["shadhin-sidebar-section-title"])
            .halign(gtk4::Align::Start)
            .build();
        pop_box.append(&title_lbl);

        let open_btn = gtk4::Button::builder()
            .label(format!("Open {} ({})", title, shortcut))
            .css_classes(["suggested-action"])
            .build();

        let pid = page_id.to_string();
        let pop_clone = popover.clone();
        let on_nav = on_navigate.clone();
        open_btn.connect_clicked(move |_| {
            pop_clone.popdown();
            if let Some(ref cb) = *on_nav.borrow() {
                cb(&pid);
            }
        });
        pop_box.append(&open_btn);
        popover.set_child(Some(&pop_box));

        let is_comp = is_compact.clone();
        let on_nav_direct = on_navigate.clone();
        let pid_direct = page_id.to_string();
        btn.connect_clicked(move |_| {
            if *is_comp.borrow() {
                popover.popup();
            } else if let Some(ref cb) = *on_nav_direct.borrow() {
                cb(&pid_direct);
            }
        });
    }

    /// Helper to create a styled sidebar button item
    fn create_sidebar_item(
        icon_name: &str,
        label_text: &str,
        is_active: bool,
    ) -> (gtk4::Button, gtk4::Label) {
        let row = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(10)
            .valign(gtk4::Align::Center)
            .build();

        let icon = gtk4::Image::from_icon_name(icon_name);
        icon.set_pixel_size(16);
        row.append(&icon);

        let label = gtk4::Label::builder()
            .label(label_text)
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .css_classes(["shadhin-sidebar-label"])
            .build();
        row.append(&label);

        let btn = gtk4::Button::builder()
            .child(&row)
            .tooltip_text(label_text)
            .focusable(true)
            .build();

        btn.add_css_class("flat");
        btn.add_css_class("shadhin-sidebar-item");
        if is_active {
            btn.add_css_class("active");
        }

        (btn, label)
    }

    /// Updates the vertical open tabs list shown in the sidebar with favicon and active state
    pub fn update_open_tabs<F: Fn(i32) + 'static>(
        &self,
        tabs: &[TabDisplayInfo],
        selected_idx: i32,
        on_select: F,
    ) {
        // Clear existing tabs
        while let Some(child) = self.tabs_list_box.first_child() {
            self.tabs_list_box.remove(&child);
        }

        let on_select = Rc::new(on_select);
        for (i, tab_data) in tabs.iter().enumerate() {
            let row = gtk4::Box::builder()
                .orientation(gtk4::Orientation::Horizontal)
                .spacing(8)
                .valign(gtk4::Align::Center)
                .build();

            // Favicon or fallback icon
            let icon = if let Some(ref texture) = tab_data.favicon {
                let img = gtk4::Image::from_paintable(Some(texture));
                img.set_pixel_size(16);
                img
            } else if tab_data.is_newtab {
                let img = gtk4::Image::from_icon_name("go-home-symbolic");
                img.set_pixel_size(16);
                img.set_opacity(0.85);
                img
            } else {
                let img = gtk4::Image::from_icon_name("globe-symbolic");
                img.set_pixel_size(16);
                img.set_opacity(0.85);
                img
            };
            row.append(&icon);

            let display_title = if tab_data.title.is_empty() {
                "New Tab".to_string()
            } else {
                tab_data.title.clone()
            };

            let label = gtk4::Label::builder()
                .label(&display_title)
                .halign(gtk4::Align::Start)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .max_width_chars(16)
                .hexpand(true)
                .css_classes(["shadhin-sidebar-label"])
                .build();
            row.append(&label);

            let btn = gtk4::Button::builder()
                .child(&row)
                .halign(gtk4::Align::Fill)
                .tooltip_text(&display_title)
                .css_classes(["flat", "shadhin-sidebar-item"])
                .build();

            if i as i32 == selected_idx {
                btn.add_css_class("active");
            }

            let sel = on_select.clone();
            btn.connect_clicked(move |_| {
                sel(i as i32);
            });
            self.tabs_list_box.append(&btn);
        }
    }

    /// Connects the callback when a sidebar page is chosen
    pub fn connect_page_selected<F: Fn(&str) + 'static>(&self, callback: F) {
        *self.on_navigate_page.borrow_mut() = Some(Box::new(callback));
    }

    /// Accessor to the root sidebar widget
    pub fn widget(&self) -> &gtk4::Box {
        &self.container
    }
}

impl Default for ShadhinSidebar {
    fn default() -> Self {
        Self::new()
    }
}
