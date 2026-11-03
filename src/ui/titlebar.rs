//! Client-side decorated title bar integrating branding, tabs, and window controls

use crate::config::SidebarState;
use crate::ui::tabbar::ShadhinTabStrip;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ShadhinTitlebar {
    container: gtk4::Box,
    tab_strip: Rc<ShadhinTabStrip>,
    sidebar_toggle_btn: gtk4::Button,
    sidebar_toggle_icon: gtk4::Image,
    private_badge: gtk4::Label,
    on_toggle_sidebar: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    on_sidebar_state_selected: Rc<RefCell<Option<Box<dyn Fn(SidebarState)>>>>,
}

impl ShadhinTitlebar {
    /// Constructs a client-side decorated title bar
    pub fn new(tab_strip: Rc<ShadhinTabStrip>) -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["shadhin-titlebar"])
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();

        let on_toggle_sidebar: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let on_sidebar_state_selected: Rc<RefCell<Option<Box<dyn Fn(SidebarState)>>>> =
            Rc::new(RefCell::new(None));

        // 1. Sidebar Toggle Button at far left
        let sidebar_toggle_icon = gtk4::Image::from_icon_name("sidebar-show-symbolic");
        sidebar_toggle_icon.set_pixel_size(16);

        let sidebar_toggle_btn = gtk4::Button::builder()
            .child(&sidebar_toggle_icon)
            .tooltip_text("Hide sidebar (Ctrl+B)")
            .css_classes(["flat", "shadhin-sidebar-toggle-btn", "active"])
            .valign(gtk4::Align::Center)
            .focusable(true)
            .build();

        // Right-click context popover to choose explicit sidebar state
        let popover = gtk4::Popover::new();
        popover.set_parent(&sidebar_toggle_btn);
        popover.set_autohide(true);

        let menu_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        let header = gtk4::Label::builder()
            .label("Sidebar Layout")
            .css_classes(["shadhin-sidebar-section-title"])
            .halign(gtk4::Align::Start)
            .build();
        menu_box.append(&header);

        let states = [
            ("Expanded (Full sidebar)", SidebarState::Expanded),
            ("Compact (Icon rail)", SidebarState::Compact),
            ("Hidden (Full screen)", SidebarState::Hidden),
        ];

        for (title, st) in states {
            let item_btn = gtk4::Button::builder()
                .label(title)
                .css_classes(["flat", "shadhin-sidebar-item"])
                .halign(gtk4::Align::Fill)
                .build();
            let pop_clone = popover.clone();
            let on_select = on_sidebar_state_selected.clone();
            item_btn.connect_clicked(move |_| {
                pop_clone.popdown();
                if let Some(ref cb) = *on_select.borrow() {
                    cb(st);
                }
            });
            menu_box.append(&item_btn);
        }
        popover.set_child(Some(&menu_box));

        // Primary left-click: toggle visible <-> hidden
        let on_toggle_clone = on_toggle_sidebar.clone();
        sidebar_toggle_btn.connect_clicked(move |_| {
            if let Some(ref cb) = *on_toggle_clone.borrow() {
                cb();
            }
        });

        // Secondary right-click: open state selection popover
        let right_click = gtk4::GestureClick::new();
        right_click.set_button(gtk4::gdk::BUTTON_SECONDARY);
        let pop_rc = popover.clone();
        right_click.connect_pressed(move |_, _, _, _| {
            pop_rc.popup();
        });
        sidebar_toggle_btn.add_controller(right_click);

        container.append(&sidebar_toggle_btn);

        // 2. Brand Logo + Title
        let brand_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(6)
            .valign(gtk4::Align::Center)
            .build();

        let logo = gtk4::Picture::for_resource("/com/shadhin/browser/icons/shadhin-logo.svg");
        logo.set_can_shrink(true);
        logo.set_width_request(24);
        logo.set_height_request(24);
        brand_box.append(&logo);

        let brand_title = gtk4::Label::builder()
            .label("Shadhin Browser")
            .css_classes(["shadhin-brand-title"])
            .valign(gtk4::Align::Center)
            .build();
        brand_box.append(&brand_title);

        let private_badge = gtk4::Label::builder()
            .label("Private")
            .css_classes(["shadhin-private-badge"])
            .valign(gtk4::Align::Center)
            .visible(false)
            .build();
        brand_box.append(&private_badge);

        container.append(&brand_box);

        // 3. Tabs Strip
        container.append(tab_strip.widget());

        // 4. Draggable Window Spacer
        let spacer = gtk4::Box::builder().hexpand(true).build();
        let drag_handle = gtk4::WindowHandle::new();
        drag_handle.set_child(Some(&spacer));
        container.append(&drag_handle);

        // 5. Standard Window Controls (Minimize, Maximize, Close)
        let controls = gtk4::WindowControls::builder()
            .side(gtk4::PackType::End)
            .decoration_layout(":minimize,maximize,close")
            .valign(gtk4::Align::Center)
            .build();
        container.append(&controls);

        Self {
            container,
            tab_strip,
            sidebar_toggle_btn,
            sidebar_toggle_icon,
            private_badge,
            on_toggle_sidebar,
            on_sidebar_state_selected,
        }
    }

    /// Toggles the visual private browsing badge in the title bar
    pub fn set_private_mode(&self, is_private: bool) {
        self.private_badge.set_visible(is_private);
    }

    /// Accessor to the tab strip
    pub fn tab_strip(&self) -> &ShadhinTabStrip {
        &self.tab_strip
    }

    /// Accessor to the root title bar widget
    pub fn widget(&self) -> &gtk4::Box {
        &self.container
    }

    /// Connects primary toggle clicked callback
    pub fn connect_sidebar_toggle<F: Fn() + 'static>(&self, callback: F) {
        *self.on_toggle_sidebar.borrow_mut() = Some(Box::new(callback));
    }

    /// Connects explicit sidebar state selection callback
    pub fn connect_sidebar_state_selected<F: Fn(SidebarState) + 'static>(&self, callback: F) {
        *self.on_sidebar_state_selected.borrow_mut() = Some(Box::new(callback));
    }

    /// Updates the toggle button visual state, icon, and tooltip
    pub fn update_sidebar_state(&self, state: SidebarState) {
        match state {
            SidebarState::Hidden => {
                self.sidebar_toggle_btn
                    .set_tooltip_text(Some("Show sidebar (Ctrl+B)"));
                self.sidebar_toggle_btn.remove_css_class("active");
                self.sidebar_toggle_icon
                    .set_icon_name(Some("sidebar-show-symbolic"));
            }
            SidebarState::Compact | SidebarState::Expanded => {
                self.sidebar_toggle_btn
                    .set_tooltip_text(Some("Hide sidebar (Ctrl+B)"));
                self.sidebar_toggle_btn.add_css_class("active");
                self.sidebar_toggle_icon
                    .set_icon_name(Some("sidebar-show-symbolic"));
            }
        }
    }
}
