//! Navigation toolbar containing back/forward/reload, address bar, and action buttons

use crate::ui::addressbar::AddressBar;
use crate::ui::privacy_panel::PrivacyPopover;
use gtk4::glib;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct ShadhinToolbar {
    container: gtk4::Box,
    back_btn: gtk4::Button,
    forward_btn: gtk4::Button,
    reload_btn: gtk4::Button,
    reload_icon: gtk4::Image,
    sidebar_toggle_btn: gtk4::Button,
    shield_btn: gtk4::Button,
    menu_btn: gtk4::MenuButton,
    address_bar: Rc<AddressBar>,
    privacy_popover: Rc<PrivacyPopover>,
    is_loading: Rc<RefCell<bool>>,
}

impl ShadhinToolbar {
    /// Constructs a new navigation toolbar
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .css_classes(["shadhin-toolbar"])
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();

        // 1. Navigation Buttons (Back, Forward, Reload)
        let back_icon = gtk4::Image::from_icon_name("go-previous-symbolic");
        let back_btn = gtk4::Button::builder()
            .child(&back_icon)
            .tooltip_text("Go back (Alt+Left)")
            .css_classes(["shadhin-nav-btn"])
            .sensitive(false)
            .build();
        container.append(&back_btn);

        let forward_icon = gtk4::Image::from_icon_name("go-next-symbolic");
        let forward_btn = gtk4::Button::builder()
            .child(&forward_icon)
            .tooltip_text("Go forward (Alt+Right)")
            .css_classes(["shadhin-nav-btn"])
            .sensitive(false)
            .build();
        container.append(&forward_btn);

        let reload_icon = gtk4::Image::from_icon_name("view-refresh-symbolic");
        let reload_btn = gtk4::Button::builder()
            .child(&reload_icon)
            .tooltip_text("Reload page (Ctrl+R)")
            .css_classes(["shadhin-nav-btn"])
            .build();
        container.append(&reload_btn);

        // 2. Pill Address Bar
        let address_bar = Rc::new(AddressBar::new());
        container.append(address_bar.widget());

        // 3. Right Action Buttons: Sidebar Toggle, Shield, Profile, Menu
        let sidebar_icon = gtk4::Image::from_icon_name("view-sidebar-symbolic");
        let sidebar_toggle_btn = gtk4::Button::builder()
            .child(&sidebar_icon)
            .tooltip_text("Toggle Sidebar (F9 or Ctrl+B)")
            .css_classes(["shadhin-nav-btn"])
            .build();
        container.append(&sidebar_toggle_btn);

        let shield_icon = gtk4::Image::from_icon_name("security-high-symbolic");
        let shield_btn = gtk4::Button::builder()
            .child(&shield_icon)
            .tooltip_text("Privacy Shield")
            .css_classes(["shadhin-nav-btn"])
            .build();
        container.append(&shield_btn);

        let privacy_popover = Rc::new(PrivacyPopover::new(&shield_btn));
        let popover_clone = privacy_popover.clone();
        shield_btn.connect_clicked(move |_| {
            popover_clone.toggle();
        });

        let profile_icon = gtk4::Image::from_icon_name("avatar-default-symbolic");
        let profile_btn = gtk4::Button::builder()
            .child(&profile_icon)
            .tooltip_text("Profile & Account (Phase 4)")
            .css_classes(["shadhin-nav-btn"])
            .build();
        profile_btn.connect_clicked(|_| {
            tracing::info!("Profile clicked (DEV-ONLY: sync arrives in Phase 4)");
        });
        container.append(&profile_btn);

        let menu_btn = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .tooltip_text("Main Menu")
            .css_classes(["shadhin-nav-btn", "flat"])
            .always_show_arrow(false)
            .build();
        container.append(&menu_btn);

        let is_loading = Rc::new(RefCell::new(false));

        Self {
            container,
            back_btn,
            forward_btn,
            reload_btn,
            reload_icon,
            sidebar_toggle_btn,
            shield_btn,
            menu_btn,
            address_bar,
            privacy_popover,
            is_loading,
        }
    }

    /// Updates back button sensitivity
    pub fn set_can_go_back(&self, can: bool) {
        self.back_btn.set_sensitive(can);
    }

    /// Updates forward button sensitivity
    pub fn set_can_go_forward(&self, can: bool) {
        self.forward_btn.set_sensitive(can);
    }

    /// Toggles Reload / Stop appearance
    pub fn set_is_loading(&self, loading: bool) {
        *self.is_loading.borrow_mut() = loading;
        if loading {
            self.reload_icon
                .set_icon_name(Some("process-stop-symbolic"));
            self.reload_btn.set_tooltip_text(Some("Stop loading"));
        } else {
            self.reload_icon
                .set_icon_name(Some("view-refresh-symbolic"));
            self.reload_btn
                .set_tooltip_text(Some("Reload page (Ctrl+R)"));
        }
    }

    /// Connects the back button clicked signal
    pub fn connect_back<F: Fn() + 'static>(&self, callback: F) {
        self.back_btn.connect_clicked(move |_| callback());
    }

    /// Connects the forward button clicked signal
    pub fn connect_forward<F: Fn() + 'static>(&self, callback: F) {
        self.forward_btn.connect_clicked(move |_| callback());
    }

    /// Connects the reload/stop button clicked signal
    pub fn connect_reload<F: Fn(bool) + 'static>(&self, callback: F) {
        let is_loading = self.is_loading.clone();
        self.reload_btn.connect_clicked(move |_| {
            let loading = *is_loading.borrow();
            callback(loading);
        });
    }

    /// Connects the sidebar toggle button clicked signal
    pub fn connect_sidebar_toggle<F: Fn() + 'static>(&self, callback: F) {
        self.sidebar_toggle_btn.connect_clicked(move |_| callback());
    }

    /// Accessor to the address bar
    pub fn address_bar(&self) -> &AddressBar {
        &self.address_bar
    }

    /// Accessor to the privacy shield popover
    pub fn privacy_popover(&self) -> &PrivacyPopover {
        &self.privacy_popover
    }

    /// Accessor to the root toolbar widget
    pub fn widget(&self) -> &gtk4::Box {
        &self.container
    }

    /// Accessor to the shield button
    pub fn shield_button(&self) -> &gtk4::Button {
        &self.shield_btn
    }

    /// Accessor to the main menu button
    pub fn menu_button(&self) -> &gtk4::MenuButton {
        &self.menu_btn
    }

    /// Sets the menu model for the main menu button
    pub fn set_menu_model<M: glib::object::IsA<gtk4::gio::MenuModel>>(&self, model: &M) {
        self.menu_btn.set_menu_model(Some(model));
    }
}

impl Default for ShadhinToolbar {
    fn default() -> Self {
        Self::new()
    }
}
