//! Main Libadwaita application window with custom styling and engine integration
//!
//! Orchestrates the client-side titlebar with embedded tabs, navigation toolbar,
//! collapsible burgundy sidebar, tab management via `TabManager` and `adw::TabView`,
//! and hardened `webkit6::WebView` instances inside a rounded content frame.

use crate::browser::downloads::DownloadManager;
use crate::browser::navigation::{NavigationTarget, classify_input};
use crate::browser::permissions::{
    PermissionRepository, PermissionState, identify_permission_request,
};
use crate::browser::session::SessionManager;
use crate::browser::tabs::{TAB_DISCARD_THRESHOLD, TabId, TabManager};
use crate::browser::webview::create_configured_webview_with_session;
use crate::config::{BrowserConfig, SidebarState, compute_effective_sidebar_state};
use crate::privacy::adblock::{ContentFilterManager, SiteExceptionRepository};
use crate::privacy::cookies::{apply_cookie_policy, apply_itp};
use crate::privacy::https_only::{HttpsMode, HttpsUpgradeManager, extract_host_from_uri};
use crate::storage::StorageService;
use crate::storage::history::HistoryRepository;
use crate::storage::workspaces::WorkspaceRepository;
use crate::ui::bookmarks_bar::BookmarksBar;
use crate::ui::bookmarks_panel::BookmarksPanel;
use crate::ui::downloads_panel::DownloadsPanel;
use crate::ui::findbar::FindBar;
use crate::ui::history_panel::HistoryPanel;
use crate::ui::newtab::ShadhinNewTab;
use crate::ui::settings::SettingsWindow;
use crate::ui::sidebar::ShadhinSidebar;
use crate::ui::tabbar::{ShadhinTabStrip, TabDisplayInfo};
use crate::ui::titlebar::ShadhinTitlebar;
use crate::ui::toolbar::ShadhinToolbar;
use crate::utils::debounce::Debounce;
use adw::prelude::*;
use gtk4::gdk::Key;
use gtk4::gio;
use gtk4::glib;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;
use tracing::info;
use webkit6::prelude::*;
use webkit6::{
    NavigationPolicyDecision, NetworkSession, PolicyDecisionType, ResponsePolicyDecision,
};

/// Representation of visual UI widgets for a browser tab
pub struct TabUiItem {
    pub stack: gtk4::Stack,
    pub webview: RefCell<Option<webkit6::WebView>>,
    pub newtab: ShadhinNewTab,
    pub bookmarks_panel: BookmarksPanel,
    pub history_panel: HistoryPanel,
    pub downloads_panel: DownloadsPanel,
    pub storage: StorageService,
    pub error_box: gtk4::Box,
    pub error_label: gtk4::Label,
    pub tls_box: gtk4::Box,
    pub tls_host_label: gtk4::Label,
    pub tls_detail_label: gtk4::Label,
    pub permission_revealer: gtk4::Revealer,
    pub permission_label: gtk4::Label,
    pub permission_icon: gtk4::Image,
    pub permission_remember: gtk4::CheckButton,
    pub permission_allow_btn: gtk4::Button,
    pub permission_block_btn: gtk4::Button,
    pub active_permission_request: RefCell<Option<webkit6::PermissionRequest>>,
    pub https_box: gtk4::Box,
    pub https_host_label: gtk4::Label,
    pub https_desc_label: gtk4::Label,
    pub https_continue_btn: gtk4::Button,
    pub https_fallback_url: RefCell<Option<String>>,
}

pub struct ShadhinWindow {
    window: adw::ApplicationWindow,
    split_view: adw::OverlaySplitView,
    titlebar: Rc<ShadhinTitlebar>,
    toolbar: Rc<ShadhinToolbar>,
    bookmarks_bar: Rc<BookmarksBar>,
    sidebar: Rc<ShadhinSidebar>,
    tab_view: adw::TabView,
    tab_strip: Rc<ShadhinTabStrip>,
    find_bar: Rc<FindBar>,
    tab_manager: Rc<RefCell<TabManager>>,
    tab_items: Rc<RefCell<Vec<Rc<TabUiItem>>>>,
    config: Rc<RefCell<BrowserConfig>>,
    storage: StorageService,
    session_mgr: Rc<RefCell<SessionManager>>,
    download_mgr: DownloadManager,
    current_workspace: Rc<RefCell<String>>,
    saved_sidebar_state: Rc<RefCell<SidebarState>>,
    effective_sidebar_state: Rc<RefCell<SidebarState>>,
    is_private: bool,
    filter_manager: ContentFilterManager,
    https_manager: Rc<RefCell<HttpsUpgradeManager>>,
    active_network_session: Rc<RefCell<NetworkSession>>,
    nav_ctx: Rc<NavigationContext>,
    persistence_allowed: bool,
}

/// Navigation execution context consolidating cross-component dependencies
#[derive(Clone)]
pub struct NavigationContext {
    pub tab_manager: Rc<RefCell<TabManager>>,
    pub config: Rc<RefCell<BrowserConfig>>,
    pub tab_items: Rc<RefCell<Vec<Rc<TabUiItem>>>>,
    pub tab_view: adw::TabView,
    pub tab_strip: Rc<ShadhinTabStrip>,
    pub sidebar: Rc<ShadhinSidebar>,
    pub toolbar: Rc<ShadhinToolbar>,
    pub find_bar: Rc<FindBar>,
    pub storage: StorageService,
    pub session: Rc<RefCell<NetworkSession>>,
    pub filter_manager: ContentFilterManager,
    pub https_manager: Rc<RefCell<HttpsUpgradeManager>>,
    pub download_mgr: DownloadManager,
    pub persistence_allowed: bool,
}

impl ShadhinWindow {
    /// Constructs and initializes the main Shadhin Browser application window
    pub fn new(app: &adw::Application) -> Self {
        Self::new_internal(app, &[], false)
    }

    /// Constructs and initializes the main Shadhin Browser window with specified URLs
    pub fn new_with_urls(app: &adw::Application, initial_urls: &[String]) -> Self {
        Self::new_internal(app, initial_urls, false)
    }

    /// Constructs and initializes a private browsing window with ephemeral session
    pub fn new_private(app: &adw::Application) -> Self {
        Self::new_internal(app, &[], true)
    }

    fn new_internal(app: &adw::Application, initial_urls: &[String], is_private: bool) -> Self {
        let config = Rc::new(RefCell::new(BrowserConfig::load()));

        // 1. Build main Libadwaita window
        let title = if is_private {
            "Shadhin Browser (Private Browsing)"
        } else {
            "Shadhin Browser"
        };
        let css_classes = if is_private {
            vec!["shadhin-window", "shadhin-private-window"]
        } else {
            vec!["shadhin-window"]
        };

        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(title)
            .default_width(1280)
            .default_height(820)
            .css_classes(css_classes)
            .build();

        // 2. Tab management and UI components
        let tab_view = adw::TabView::new();
        let tab_strip = Rc::new(ShadhinTabStrip::new(&tab_view));
        let titlebar = Rc::new(ShadhinTitlebar::new(tab_strip.clone()));
        if is_private {
            titlebar.set_private_mode(true);
        }
        let toolbar = Rc::new(ShadhinToolbar::new());
        let sidebar = Rc::new(ShadhinSidebar::new());
        let find_bar = Rc::new(FindBar::new());

        let tab_manager = Rc::new(RefCell::new(TabManager::new()));
        let tab_items: Rc<RefCell<Vec<Rc<TabUiItem>>>> = Rc::new(RefCell::new(Vec::new()));

        // 3. Central Content Box (Titlebar + Toolbar + Content Frame)
        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        content_box.append(titlebar.widget());
        content_box.append(toolbar.widget());

        // Rounded Content Frame hosting TabView and floating FindBar via Overlay
        let content_frame = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(["shadhin-content-frame"])
            .hexpand(true)
            .vexpand(true)
            .build();

        tab_view.set_hexpand(true);
        tab_view.set_vexpand(true);

        let overlay = gtk4::Overlay::new();
        overlay.set_hexpand(true);
        overlay.set_vexpand(true);
        overlay.set_child(Some(&tab_view));
        overlay.add_overlay(find_bar.widget());

        content_frame.append(&overlay);
        content_box.append(&content_frame);

        // 4. Persistence, sessions, and downloads initialization
        let (storage, persistence_allowed) = if is_private {
            let st = StorageService::new_in_memory().unwrap_or_else(|e| {
                tracing::error!("Failed to initialize in-memory storage: {e}");
                StorageService::open_default().unwrap()
            });
            (st, false)
        } else {
            let st = StorageService::open_default().unwrap_or_else(|e| {
                tracing::warn!("Failed to open default storage ({e}); using in-memory database");
                StorageService::new_in_memory().unwrap_or_else(|e2| {
                    tracing::error!("Failed to create in-memory database: {e2}");
                    panic!("Cannot initialize any storage backend: {e2}");
                })
            });
            (st, true)
        };

        let session_mgr = Rc::new(RefCell::new(SessionManager::new(storage.clone())));
        let active_sess_inst = if is_private {
            NetworkSession::new_ephemeral()
        } else {
            session_mgr
                .borrow_mut()
                .get_or_create_workspace_session("default")
        };
        let active_network_session = Rc::new(RefCell::new(active_sess_inst));

        let dl_dir = PathBuf::from(&config.borrow().download_dir);
        let download_mgr = DownloadManager::new(storage.clone(), dl_dir);
        download_mgr.attach_session(&active_network_session.borrow(), persistence_allowed);

        apply_cookie_policy(
            &active_network_session.borrow(),
            crate::privacy::cookies::CookiePolicy::BlockThirdParty,
        );
        apply_itp(&active_network_session.borrow(), true);

        let filter_manager = ContentFilterManager::new();
        filter_manager.initialize(|| {
            info!("Bundled content filter lists compiled & ready");
        });

        let https_manager = Rc::new(RefCell::new(HttpsUpgradeManager::new()));

        toolbar.address_bar().set_storage(storage.clone());
        let bookmarks_bar = Rc::new(BookmarksBar::new(storage.clone()));
        content_box.insert_child_after(bookmarks_bar.widget(), Some(toolbar.widget()));
        let current_workspace = Rc::new(RefCell::new("default".to_string()));

        // Banners
        if is_private {
            let pvt_banner = adw::Banner::builder()
                .title("Private Browsing: Shadhin won't save browsing history, cookies, or site data. Downloaded files remain on disk.")
                .revealed(true)
                .build();
            content_box.prepend(&pvt_banner);
        } else if session_mgr.borrow().had_unclean_exit() {
            let banner = adw::Banner::builder()
                .title("Shadhin Browser closed unexpectedly.")
                .button_label("Restore Session")
                .revealed(true)
                .build();

            let sm_click = session_mgr.clone();
            let banner_ref = banner.clone();
            banner.connect_button_clicked(move |_| {
                sm_click.borrow().acknowledge_crash();
                banner_ref.set_revealed(false);
            });
            content_box.prepend(&banner);
        }

        // 5. Overlay Split View (Sidebar + Content)
        let split_view = adw::OverlaySplitView::new();
        split_view.set_sidebar(Some(sidebar.widget()));
        split_view.set_content(Some(&content_box));
        split_view.set_hexpand(true);
        split_view.set_vexpand(true);

        let initial_pref = config.borrow().sidebar_state;
        let initial_eff = compute_effective_sidebar_state(1280, initial_pref);

        let saved_sidebar_state = Rc::new(RefCell::new(initial_pref));
        let effective_sidebar_state = Rc::new(RefCell::new(initial_eff));

        Self::apply_sidebar_state(
            initial_eff,
            &split_view,
            &sidebar,
            &titlebar,
            &effective_sidebar_state,
        );

        // Responsive breakpoints:
        // Breakpoint for < 900px (collapses Expanded -> Compact)
        let bp_compact = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 900px").expect("valid condition"),
        );
        let sv_bp = split_view.clone();
        let sb_bp = sidebar.clone();
        let tb_bp = titlebar.clone();
        let eff_bp = effective_sidebar_state.clone();
        let saved_bp = saved_sidebar_state.clone();
        let win_bp_clone = window.clone();
        bp_compact.connect_apply(move |_| {
            let cur_w = win_bp_clone.width();
            let saved = *saved_bp.borrow();
            let effective = compute_effective_sidebar_state(cur_w, saved);
            Self::apply_sidebar_state(effective, &sv_bp, &sb_bp, &tb_bp, &eff_bp);
        });
        let sv_unbp = split_view.clone();
        let sb_unbp = sidebar.clone();
        let tb_unbp = titlebar.clone();
        let eff_unbp = effective_sidebar_state.clone();
        let saved_unbp = saved_sidebar_state.clone();
        let win_unbp_clone = window.clone();
        bp_compact.connect_unapply(move |_| {
            let cur_w = win_unbp_clone.width();
            let saved = *saved_unbp.borrow();
            let effective = compute_effective_sidebar_state(cur_w, saved);
            Self::apply_sidebar_state(effective, &sv_unbp, &sb_unbp, &tb_unbp, &eff_unbp);
        });
        window.add_breakpoint(bp_compact);

        // Breakpoint for < 650px (collapses to Hidden)
        let bp_hidden = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 650px").expect("valid condition"),
        );
        let sv_bph = split_view.clone();
        let sb_bph = sidebar.clone();
        let tb_bph = titlebar.clone();
        let eff_bph = effective_sidebar_state.clone();
        bp_hidden.connect_apply(move |_| {
            Self::apply_sidebar_state(SidebarState::Hidden, &sv_bph, &sb_bph, &tb_bph, &eff_bph);
        });
        let sv_unbph = split_view.clone();
        let sb_unbph = sidebar.clone();
        let tb_unbph = titlebar.clone();
        let eff_unbph = effective_sidebar_state.clone();
        let saved_unbph = saved_sidebar_state.clone();
        let win_unbph_clone = window.clone();
        bp_hidden.connect_unapply(move |_| {
            let cur_w = win_unbph_clone.width();
            let saved = *saved_unbph.borrow();
            let effective = compute_effective_sidebar_state(cur_w, saved);
            Self::apply_sidebar_state(effective, &sv_unbph, &sb_unbph, &tb_unbph, &eff_unbph);
        });
        window.add_breakpoint(bp_hidden);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&split_view));
        window.set_content(Some(&toast_overlay));

        let toast_ov = toast_overlay.clone();
        download_mgr.add_completion_listener(move |item| {
            let toast = adw::Toast::new(&format!("Download completed: {}", item.file_name));
            toast.set_button_label(Some("Open"));
            let file_path = item.file_path.clone();
            toast.connect_button_clicked(move |_| {
                let uri = format!("file://{}", file_path.display());
                let _ = gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>);
            });
            toast_ov.add_toast(toast);
        });

        let nav_ctx = Rc::new(NavigationContext {
            tab_manager: tab_manager.clone(),
            config: config.clone(),
            tab_items: tab_items.clone(),
            tab_view: tab_view.clone(),
            tab_strip: tab_strip.clone(),
            sidebar: sidebar.clone(),
            toolbar: toolbar.clone(),
            find_bar: find_bar.clone(),
            storage: storage.clone(),
            session: active_network_session.clone(),
            filter_manager: filter_manager.clone(),
            https_manager: https_manager.clone(),
            download_mgr: download_mgr.clone(),
            persistence_allowed,
        });

        let win = Self {
            window,
            split_view,
            titlebar,
            toolbar,
            bookmarks_bar,
            sidebar,
            tab_view,
            tab_strip,
            find_bar,
            tab_manager,
            tab_items,
            config,
            storage,
            session_mgr,
            download_mgr,
            current_workspace,
            saved_sidebar_state,
            effective_sidebar_state,
            is_private,
            filter_manager,
            https_manager,
            active_network_session,
            nav_ctx,
            persistence_allowed,
        };

        win.setup_signals();
        win.setup_shortcuts();

        if persistence_allowed {
            let storage_clean = win.storage.clone();
            let retention_debounce = Debounce::new();
            retention_debounce.debounce(Duration::from_secs(5), move || {
                storage_clean.dispatch(|db| {
                    let repo = HistoryRepository::new(db.conn());
                    let _ = repo.cleanup_retention(90);
                });
            });
        }

        // Open initial tabs (either landing New Tab or requested URLs)
        if initial_urls.is_empty() {
            win.create_tab(None, None);
        } else {
            for url in initial_urls {
                win.create_tab(Some(url), None);
            }
        }

        win
    }

    /// Synchronizes the visual tab strip, sidebar open tabs list, and toolbar state
    fn sync_tabs(&self) {
        self.nav_ctx.sync_tabs();
    }

    /// Creates and configures a new tab page with lazy WebView allocation
    pub fn create_tab(&self, initial_url: Option<&str>, parent_id: Option<TabId>) -> usize {
        self.nav_ctx.create_tab(initial_url, parent_id)
    }

    /// Standalone helper to trigger UI tab strip and sidebar synchronization
    fn sync_tabs_external(
        tm_cell: &Rc<RefCell<TabManager>>,
        ts: &Rc<ShadhinTabStrip>,
        sb: &Rc<ShadhinSidebar>,
        tv: &adw::TabView,
        tb: &Rc<ShadhinToolbar>,
    ) {
        let tm = tm_cell.borrow();
        let tabs_info: Vec<TabDisplayInfo> = tm
            .tabs()
            .iter()
            .map(|t| {
                let b = t.borrow();
                TabDisplayInfo {
                    title: b.title.clone(),
                    favicon: b.favicon.clone(),
                    is_newtab: b.is_newtab(),
                    is_discarded: b.is_discarded,
                }
            })
            .collect();

        ts.refresh(&tabs_info);

        let sel_idx = tm.selected_index();
        let tv_clone = tv.clone();
        sb.update_open_tabs(&tabs_info, sel_idx as i32, move |idx| {
            if idx >= 0 && idx < tv_clone.n_pages() {
                let page = tv_clone.nth_page(idx);
                tv_clone.set_selected_page(&page);
            }
        });

        if let Some(active_tab_cell) = tm.active_tab() {
            let active = active_tab_cell.borrow();
            if active.is_newtab() {
                tb.address_bar().set_text_if_unfocused("");
                tb.address_bar().set_security_state(true);
            } else {
                tb.address_bar().set_text_if_unfocused(&active.url);
                tb.address_bar()
                    .set_security_state(active.url.starts_with("https://"));
            }
        }
    }
}

impl NavigationContext {
    pub fn create_tab(&self, initial_url: Option<&str>, parent_id: Option<TabId>) -> usize {
        let (tab_idx, _tab_model) = self
            .tab_manager
            .borrow_mut()
            .new_tab(initial_url, parent_id);

        let stack = gtk4::Stack::new();
        stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        stack.set_hexpand(true);
        stack.set_vexpand(true);

        // 1. Native New Tab view
        let newtab = ShadhinNewTab::new();
        stack.add_named(newtab.widget(), Some("newtab"));

        // 2. Styled Network Error view
        let error_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .css_classes(["shadhin-error-box"])
            .build();

        let error_icon = gtk4::Image::from_icon_name("network-error-symbolic");
        error_icon.set_pixel_size(48);
        error_box.append(&error_icon);

        let error_title = gtk4::Label::builder()
            .label("Unable to Connect")
            .css_classes(["shadhin-error-title"])
            .build();
        error_box.append(&error_title);

        let error_label = gtk4::Label::builder()
            .label("Check your network connection and try again.")
            .css_classes(["shadhin-error-desc"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .build();
        error_box.append(&error_label);

        let retry_btn = gtk4::Button::builder()
            .label("Try Again")
            .css_classes(["suggested-action", "shadhin-nav-btn"])
            .margin_top(8)
            .build();
        error_box.append(&retry_btn);
        stack.add_named(&error_box, Some("error"));

        // 3. Styled TLS Certificate Warning view
        let tls_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .css_classes(["shadhin-tls-box"])
            .build();

        let tls_icon = gtk4::Image::from_icon_name("dialog-warning-symbolic");
        tls_icon.set_pixel_size(52);
        tls_box.append(&tls_icon);

        let tls_title = gtk4::Label::builder()
            .label("Your connection is not private")
            .css_classes(["shadhin-tls-title"])
            .build();
        tls_box.append(&tls_title);

        let tls_host_label = gtk4::Label::builder()
            .label("")
            .css_classes(["shadhin-tls-desc"])
            .build();
        tls_box.append(&tls_host_label);

        let tls_detail_label = gtk4::Label::builder()
            .label(
                "The security certificate presented by this website has errors or is not trusted.",
            )
            .css_classes(["shadhin-tls-desc"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .build();
        tls_box.append(&tls_detail_label);

        let tls_actions = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .build();

        let back_safety_btn = gtk4::Button::builder()
            .label("Go Back to Safety")
            .css_classes(["suggested-action", "shadhin-nav-btn"])
            .build();
        tls_actions.append(&back_safety_btn);
        tls_box.append(&tls_actions);
        stack.add_named(&tls_box, Some("tls"));

        // 4. HTTPS-Only Fallback Interstitial view
        let https_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(16)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .css_classes(["shadhin-https-box"])
            .build();

        let https_icon = gtk4::Image::from_icon_name("security-low-symbolic");
        https_icon.set_pixel_size(48);
        https_box.append(&https_icon);

        let https_title = gtk4::Label::builder()
            .label("Secure Connection Not Available")
            .css_classes(["shadhin-https-title"])
            .build();
        https_box.append(&https_title);

        let https_host_label = gtk4::Label::builder()
            .css_classes(["shadhin-https-host"])
            .build();
        https_box.append(&https_host_label);

        let https_desc = gtk4::Label::builder()
            .label("HTTPS-Only Mode is enabled. The site you are attempting to visit does not support HTTPS.")
            .css_classes(["shadhin-https-desc"])
            .wrap(true)
            .justify(gtk4::Justification::Center)
            .build();
        https_box.append(&https_desc);

        let https_actions = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .halign(gtk4::Align::Center)
            .margin_top(8)
            .build();

        let https_back_btn = gtk4::Button::builder()
            .label("Go Back")
            .css_classes(["suggested-action", "shadhin-nav-btn"])
            .build();
        let https_continue_btn = gtk4::Button::builder()
            .label("Continue to HTTP Site")
            .css_classes(["destructive-action", "shadhin-nav-btn"])
            .build();
        https_actions.append(&https_back_btn);
        https_actions.append(&https_continue_btn);
        https_box.append(&https_actions);
        stack.add_named(&https_box, Some("https_error"));

        // 5. Non-modal Permission Request Bar
        let permission_revealer = gtk4::Revealer::new();
        permission_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        let perm_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(12)
            .css_classes(["shadhin-permission-bar"])
            .build();
        let permission_icon = gtk4::Image::from_icon_name("dialog-question-symbolic");
        permission_icon.set_pixel_size(20);
        let permission_label = gtk4::Label::builder()
            .label("This site wants access to a device feature")
            .hexpand(true)
            .xalign(0.0)
            .build();
        let permission_remember = gtk4::CheckButton::with_label("Remember decision");
        permission_remember.set_active(true);
        let permission_allow_btn = gtk4::Button::builder()
            .label("Allow")
            .css_classes(["suggested-action", "pill"])
            .build();
        let permission_block_btn = gtk4::Button::builder()
            .label("Block")
            .css_classes(["pill"])
            .build();
        perm_box.append(&permission_icon);
        perm_box.append(&permission_label);
        perm_box.append(&permission_remember);
        perm_box.append(&permission_allow_btn);
        perm_box.append(&permission_block_btn);
        permission_revealer.set_child(Some(&perm_box));
        permission_revealer.set_reveal_child(false);

        // 6. Native Bookmarks, History, Downloads Panels
        let bookmarks_panel = BookmarksPanel::new(self.storage.clone());
        stack.add_named(bookmarks_panel.widget(), Some("bookmarks"));

        let history_panel = HistoryPanel::new(self.storage.clone());
        stack.add_named(history_panel.widget(), Some("history"));

        let downloads_panel = DownloadsPanel::new(
            self.storage.clone(),
            self.download_mgr.clone(),
            self.session.clone(),
            self.persistence_allowed,
        );
        stack.add_named(downloads_panel.widget(), Some("downloads"));

        newtab.load_data(self.storage.clone());

        let ui_item = Rc::new(TabUiItem {
            stack: stack.clone(),
            webview: RefCell::new(None),
            newtab,
            bookmarks_panel,
            history_panel,
            downloads_panel,
            storage: self.storage.clone(),
            error_box,
            error_label,
            tls_box,
            tls_host_label,
            tls_detail_label,
            permission_revealer: permission_revealer.clone(),
            permission_label,
            permission_icon,
            permission_remember: permission_remember.clone(),
            permission_allow_btn: permission_allow_btn.clone(),
            permission_block_btn: permission_block_btn.clone(),
            active_permission_request: RefCell::new(None),
            https_box,
            https_host_label: https_host_label.clone(),
            https_desc_label: https_desc.clone(),
            https_continue_btn: https_continue_btn.clone(),
            https_fallback_url: RefCell::new(None),
        });

        self.tab_items.borrow_mut().push(ui_item.clone());

        let page_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();
        page_container.append(&permission_revealer);
        page_container.append(&stack);

        let page = self.tab_view.append(&page_container);
        page.set_title("New Tab");
        self.tab_view.set_selected_page(&page);

        // Wire NewTab navigation callback
        let nav_nt = self.clone();
        let page_nt = page.clone();
        ui_item.newtab.set_on_navigate(move |input| {
            info!("NewTab navigating to: {input}");
            let target = classify_input(input, &nav_nt.config.borrow().search_engine_url);
            nav_nt.execute_navigation(tab_idx, &target, &page_nt);
        });

        // If initial_url was provided and not a newtab page, initialize the WebView and load
        if let Some(url) = initial_url {
            if url != "about:newtab" && url != "shadhin:newtab" {
                let target = classify_input(url, &self.config.borrow().search_engine_url);
                self.execute_navigation(tab_idx, &target, &page);
            } else {
                stack.set_visible_child_name("newtab");
            }
        } else {
            stack.set_visible_child_name("newtab");
        }

        // Wire Retry button on Error view
        let nav_retry = self.clone();
        let page_retry = page.clone();
        retry_btn.connect_clicked(move |_| {
            let tm = nav_retry.tab_manager.borrow();
            if let Some(tab_cell) = tm.get_tab(tab_idx) {
                let url = tab_cell.borrow().url.clone();
                let target = classify_input(&url, &nav_retry.config.borrow().search_engine_url);
                drop(tm);
                nav_retry.execute_navigation(tab_idx, &target, &page_retry);
            }
        });

        // Wire Safety button on TLS view (Go back or open New Tab)
        let nav_tls_back = self.clone();
        let stack_tls = stack.clone();
        back_safety_btn.connect_clicked(move |_| {
            let items = nav_tls_back.tab_items.borrow();
            if let Some(item) = items.get(tab_idx)
                && let Some(ref wv) = *item.webview.borrow()
                && wv.can_go_back()
            {
                wv.go_back();
                stack_tls.set_visible_child_name("web");
                return;
            }
            let tm = nav_tls_back.tab_manager.borrow();
            if let Some(tab_cell) = tm.get_tab(tab_idx) {
                let mut tab = tab_cell.borrow_mut();
                tab.url = "about:newtab".to_string();
                tab.title = "New Tab".to_string();
            }
            stack_tls.set_visible_child_name("newtab");
        });

        // Wire HTTPS Go Back button
        let nav_hb = self.clone();
        let stack_hb = stack.clone();
        https_back_btn.connect_clicked(move |_| {
            let items = nav_hb.tab_items.borrow();
            if let Some(item) = items.get(tab_idx)
                && let Some(ref wv) = *item.webview.borrow()
                && wv.can_go_back()
            {
                wv.go_back();
                stack_hb.set_visible_child_name("web");
                return;
            }
            let tm = nav_hb.tab_manager.borrow();
            if let Some(tab_cell) = tm.get_tab(tab_idx) {
                let mut tab = tab_cell.borrow_mut();
                tab.url = "about:newtab".to_string();
                tab.title = "New Tab".to_string();
            }
            stack_hb.set_visible_child_name("newtab");
        });

        // Wire HTTPS Continue to HTTP Site button
        let nav_hc = self.clone();
        let ui_hc = ui_item.clone();
        let host_hc = https_host_label.clone();
        https_continue_btn.connect_clicked(move |_| {
            let fallback_url_opt = ui_hc.https_fallback_url.borrow().clone();
            let url_to_load = if let Some(ref fallback) = fallback_url_opt {
                fallback.clone()
            } else {
                let host = host_hc.text().to_string();
                if host.is_empty() {
                    return;
                }
                format!("http://{host}")
            };

            if let Some(host) = extract_host_from_uri(&url_to_load) {
                nav_hc
                    .https_manager
                    .borrow_mut()
                    .add_session_exemption(&host);
            }

            if let Some(ref wv) = *ui_hc.webview.borrow() {
                wv.load_uri(&url_to_load);
                ui_hc.stack.set_visible_child_name("web");
            }
        });

        // Wire Permission Allow button
        let nav_pa = self.clone();
        let ui_pa = ui_item.clone();
        let rev_pa = permission_revealer.clone();
        permission_allow_btn.connect_clicked(move |_| {
            if let Some(req) = ui_pa.active_permission_request.borrow_mut().take() {
                let (ptype, _) = identify_permission_request(&req);
                let remember = ui_pa.permission_remember.is_active();
                if remember
                    && nav_pa.persistence_allowed
                    && let Some(ref wv) = *ui_pa.webview.borrow()
                    && let Some(uri) = wv.uri()
                    && let Some(host) = extract_host_from_uri(uri.as_str())
                {
                    nav_pa.storage.dispatch(move |db| {
                        let repo = PermissionRepository::new(db.conn());
                        let _ = repo.set_permission(
                            &host,
                            ptype,
                            PermissionState::Allow,
                            nav_pa.persistence_allowed,
                        );
                    });
                }
                req.allow();
                rev_pa.set_reveal_child(false);
            }
        });

        // Wire Permission Block button
        let nav_pb = self.clone();
        let ui_pb = ui_item.clone();
        let rev_pb = permission_revealer.clone();
        permission_block_btn.connect_clicked(move |_| {
            if let Some(req) = ui_pb.active_permission_request.borrow_mut().take() {
                let (ptype, _) = identify_permission_request(&req);
                let remember = ui_pb.permission_remember.is_active();
                if remember
                    && nav_pb.persistence_allowed
                    && let Some(ref wv) = *ui_pb.webview.borrow()
                    && let Some(uri) = wv.uri()
                    && let Some(host) = extract_host_from_uri(uri.as_str())
                {
                    nav_pb.storage.dispatch(move |db| {
                        let repo = PermissionRepository::new(db.conn());
                        let _ = repo.set_permission(
                            &host,
                            ptype,
                            PermissionState::Block,
                            nav_pb.persistence_allowed,
                        );
                    });
                }
                req.deny();
                rev_pb.set_reveal_child(false);
            }
        });

        self.sync_tabs();
        tab_idx
    }

    pub fn setup_webview_for_tab(&self, tab_idx: usize, page: &adw::TabPage) -> webkit6::WebView {
        let js_enabled = self.config.borrow().privacy.enable_javascript;
        let session = self.session.borrow();
        let webview = create_configured_webview_with_session(js_enabled, Some(&*session));

        // Attach content filter rules
        let ucm = webview.user_content_manager().expect("UserContentManager");
        let shield_enabled = self.config.borrow().privacy.shield_enabled;
        let ads_enabled = self.config.borrow().privacy.block_ads;
        let trackers_enabled = self.config.borrow().privacy.block_trackers;
        self.filter_manager
            .apply_filters(&ucm, shield_enabled, ads_enabled, trackers_enabled);

        // Attach to stack safely with bounds check
        let items = self.tab_items.borrow();
        let Some(ui_item) = items.get(tab_idx).cloned() else {
            tracing::warn!(
                "tab_items index {tab_idx} out of bounds (len: {})",
                items.len()
            );
            return webview;
        };
        ui_item.stack.add_named(&webview, Some("web"));
        *ui_item.webview.borrow_mut() = Some(webview.clone());
        let storage_lc = ui_item.storage.clone();
        drop(items);

        let Some(tab_c) = self.tab_manager.borrow().get_tab(tab_idx) else {
            tracing::warn!("tab_manager index {tab_idx} out of bounds");
            return webview;
        };
        let tab_id = tab_c.borrow().id;

        self.find_bar.attach_webview(&webview);

        // 1. connect_load_changed
        let nav_lc = self.clone();
        let page_lc = page.clone();
        let tab_c_lc = tab_c.clone();
        let tb_lc = self.toolbar.clone();

        webview.connect_load_changed(move |wv, event| {
            let mut tab = tab_c_lc.borrow_mut();
            match event {
                webkit6::LoadEvent::Started => {
                    tab.is_loading = true;
                    tb_lc.set_is_loading(true);
                    if let Some(uri) = wv.uri() {
                        let uri_str = uri.as_str();
                        tab.url = uri_str.to_string();
                        tb_lc.address_bar().set_text_if_unfocused(uri_str);
                    }
                }
                webkit6::LoadEvent::Committed => {
                    nav_lc
                        .https_manager
                        .borrow_mut()
                        .clear_upgrade_attempt(tab_id);
                    if let Some(uri) = wv.uri() {
                        let uri_str = uri.as_str();
                        tab.url = uri_str.to_string();
                        tb_lc.address_bar().set_text_if_unfocused(uri_str);
                        if uri_str.starts_with("https://") {
                            tb_lc.address_bar().set_security_status(
                                "security-high-symbolic",
                                "Connection is secure (HTTPS)",
                            );
                        } else if uri_str.starts_with("http://") {
                            tb_lc.address_bar().set_security_status(
                                "channel-insecure-symbolic",
                                "Connection is not secure (HTTP)",
                            );
                        } else {
                            tb_lc.address_bar().set_security_status(
                                "security-low-symbolic",
                                "Internal application page",
                            );
                        }
                        if let Some(host) = extract_host_from_uri(uri_str) {
                            tb_lc.privacy_popover().update_for_site(Some(&host), true);
                        }
                    }
                    tab.can_go_back = wv.can_go_back();
                    tab.can_go_forward = wv.can_go_forward();
                    tb_lc.set_can_go_back(tab.can_go_back);
                    tb_lc.set_can_go_forward(tab.can_go_forward);
                }
                webkit6::LoadEvent::Finished => {
                    tab.is_loading = false;
                    tab.progress = 0.0;
                    tb_lc.set_is_loading(false);
                    tb_lc.address_bar().set_progress(0.0);
                    if let Some(title) = wv.title() {
                        let title_str = title.to_string();
                        tab.title = title_str.clone();
                        page_lc.set_title(&title_str);
                    }
                    tab.favicon = wv.favicon();
                    tab.can_go_back = wv.can_go_back();
                    tab.can_go_forward = wv.can_go_forward();
                    tb_lc.set_can_go_back(tab.can_go_back);
                    tb_lc.set_can_go_forward(tab.can_go_forward);

                    let current_url = tab.url.clone();
                    let current_title = tab.title.clone();

                    drop(tab);

                    crate::utils::net_diagnostics::record_load_finished(&current_url);

                    // Record visit to history for web URLs if persistence is allowed
                    if nav_lc.persistence_allowed
                        && (current_url.starts_with("http://")
                            || current_url.starts_with("https://"))
                    {
                        let st = storage_lc.clone();
                        st.dispatch(move |db| {
                            let repo = HistoryRepository::new(db.conn());
                            let _ = repo.record_visit(&current_url, &current_title, true);
                        });
                    }

                    nav_lc.sync_tabs();
                }
                _ => {}
            }
        });

        // 2. connect_title_notify
        let nav_title = self.clone();
        let page_title = page.clone();
        let tab_c_title = tab_c.clone();
        webview.connect_title_notify(move |wv| {
            if let Some(t) = wv.title() {
                let s = t.to_string();
                tab_c_title.borrow_mut().title = s.clone();
                page_title.set_title(&s);
                nav_title.sync_tabs();
            }
        });

        // 3. connect_favicon_notify
        let nav_fav = self.clone();
        let tab_c_fav = tab_c.clone();
        webview.connect_favicon_notify(move |wv| {
            tab_c_fav.borrow_mut().favicon = wv.favicon();
            nav_fav.sync_tabs();
        });

        // 4. connect_estimated_load_progress_notify
        let tb_prog = self.toolbar.clone();
        let tab_c_prog = tab_c.clone();
        webview.connect_estimated_load_progress_notify(move |wv| {
            let progress = wv.estimated_load_progress();
            tab_c_prog.borrow_mut().progress = progress;
            tb_prog.address_bar().set_progress(progress);
        });

        // 5. connect_decide_policy (HTTPS-Only Upgrade interceptor & Net Diagnostics)
        let https_mgr_dp = self.https_manager.clone();
        let cfg_dp = self.config.clone();
        let ui_dp = ui_item.clone();
        let tab_c_dp = tab_c.clone();
        let tb_dp = self.toolbar.clone();
        let page_dp = page.clone();

        webview.connect_decide_policy(move |wv, decision, decision_type| {
            if decision_type == PolicyDecisionType::Response
                && let Ok(resp_decision) = decision.clone().downcast::<ResponsePolicyDecision>()
            {
                let is_main_frame = resp_decision.is_main_frame_main_resource();
                if let Some(resp) = resp_decision.response() {
                    crate::utils::net_diagnostics::record_response_decision(&resp, is_main_frame);
                }

                let is_supported = resp_decision.is_mime_type_supported();
                let is_attachment = resp_decision
                    .response()
                    .and_then(|r| r.http_headers())
                    .map(|h| {
                        if let Some((disposition, _)) = h.content_disposition() {
                            disposition.eq_ignore_ascii_case("attachment")
                        } else if let Some(cd) = h.one("Content-Disposition") {
                            cd.to_ascii_lowercase().contains("attachment")
                        } else {
                            false
                        }
                    })
                    .unwrap_or(false);

                if !is_supported || is_attachment {
                    info!(
                        "ResponsePolicyDecision: routing to download manager (supported={is_supported}, attachment={is_attachment})"
                    );
                    decision.download();
                    return true;
                }
            }

            if decision_type == PolicyDecisionType::NavigationAction
                && let Ok(nav_decision) = decision.clone().downcast::<NavigationPolicyDecision>()
                && let Some(action) = nav_decision.navigation_action()
                && let Some(req) = action.request()
                && let Some(uri) = req.uri()
            {
                let is_redirect = action.is_redirect();
                crate::utils::net_diagnostics::record_navigation_request(
                    &req,
                    is_redirect,
                    None,
                );

                let uri_str = uri.as_str();
                // Block web content from navigating to internal application schemes or local filesystem
                if uri_str.starts_with("shadhin:") || uri_str.starts_with("file:") {
                    tracing::warn!("Blocked attempt by web content to navigate to internal/local URI: {uri_str}");
                    decision.ignore();
                    return true;
                }

                if uri_str.starts_with("http://")
                    && let Some(host) = extract_host_from_uri(uri_str)
                {
                    let mode = cfg_dp.borrow().privacy.resolved_https_mode();

                    // Redirect loop detection: if this tab is already attempting an upgrade
                    // and the site redirects back to the original insecure URI, stop and show interstitial
                    if https_mgr_dp.borrow().is_redirect_loop(tab_id, uri_str) {
                        tracing::warn!(
                            "Detected HTTPS upgrade redirect loop for tab {tab_id} at {uri_str}"
                        );
                        decision.ignore();
                        https_mgr_dp.borrow_mut().clear_upgrade_attempt(tab_id);

                        ui_dp
                            .https_fallback_url
                            .borrow_mut()
                            .replace(uri_str.to_string());
                        ui_dp.https_host_label.set_label(&host);

                        if mode == HttpsMode::Strict {
                            ui_dp.https_desc_label.set_label(
                                "HTTPS-Only Mode is set to Strict. Insecure HTTP connections are blocked and cannot be bypassed.",
                            );
                            ui_dp.https_continue_btn.set_visible(false);
                        } else {
                            ui_dp.https_desc_label.set_label(
                                "This website is repeatedly redirecting to an insecure HTTP connection.",
                            );
                            ui_dp.https_continue_btn.set_visible(true);
                        }

                        tb_dp.set_is_loading(false);
                        page_dp.set_title("Secure Connection Not Available");
                        let mut tab = tab_c_dp.borrow_mut();
                        tab.is_loading = false;
                        tab.title = "Secure Connection Not Available".to_string();
                        drop(tab);

                        ui_dp.stack.set_visible_child_name("https_error");
                        return true;
                    }

                    if https_mgr_dp.borrow().should_upgrade(uri_str, mode)
                        && let Some(upgraded) =
                            crate::privacy::https_only::upgrade_uri_to_https(uri_str)
                    {
                        https_mgr_dp.borrow_mut().record_upgrade_attempt(
                            tab_id,
                            uri_str,
                            &upgraded,
                            &host,
                        );
                        info!("HTTPS-Only upgrading (tab {tab_id}) {uri_str} -> {upgraded}");

                        // Ignore HTTP navigation first
                        decision.ignore();

                        // Schedule the HTTPS navigation asynchronously on the GTK main loop
                        // to avoid re-entrant navigation while the policy decision is pending!
                        let wv_clone = wv.clone();
                        glib::idle_add_local_once(move || {
                            wv_clone.load_uri(&upgraded);
                        });
                        return true;
                    }
                }
            }
            false
        });

        // 6. connect_load_failed (with HTTPS-Only Interstitial fallback & subresource isolation)
        let ui_fail = ui_item.clone();
        let tb_fail = self.toolbar.clone();
        let page_fail = page.clone();
        let tab_c_fail = tab_c.clone();
        let https_mgr_lf = self.https_manager.clone();
        let cfg_fail = self.config.clone();

        webview.connect_load_failed(move |wv, load_event, failing_uri, error| {
            crate::utils::net_diagnostics::record_load_failed(failing_uri, load_event, error);

            // Differentiate cancelled loads resulting from policy interventions (decision.ignore)
            if crate::browser::webview::is_cancellation_error(error) {
                tracing::debug!(
                    "Ignoring cancelled load for {failing_uri} (event: {load_event:?})"
                );
                return true;
            }

            let tab_url = tab_c_fail.borrow().url.clone();
            let wv_uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();

            // Check if this failure is for an active HTTPS upgrade attempt on this tab
            let is_upgrade_target = https_mgr_lf
                .borrow()
                .get_pending_upgrade(tab_id)
                .map(|p| p.upgraded_https_uri == failing_uri || p.original_http_uri == failing_uri)
                .unwrap_or(false);

            if is_upgrade_target
                && let Some(pending) = https_mgr_lf.borrow_mut().clear_upgrade_attempt(tab_id)
            {
                tb_fail.set_is_loading(false);
                page_fail.set_title("Secure Connection Not Available");
                let mut tab = tab_c_fail.borrow_mut();
                tab.is_loading = false;
                tab.title = "Secure Connection Not Available".to_string();
                drop(tab);

                let mode = cfg_fail.borrow().privacy.resolved_https_mode();
                ui_fail
                    .https_fallback_url
                    .borrow_mut()
                    .replace(pending.original_http_uri.clone());
                ui_fail.https_host_label.set_label(&pending.host);

                if mode == HttpsMode::Strict {
                    ui_fail.https_desc_label.set_label(
                        "HTTPS-Only Mode is set to Strict. Insecure HTTP connections are blocked and cannot be bypassed.",
                    );
                    ui_fail.https_continue_btn.set_visible(false);
                } else {
                    ui_fail.https_desc_label.set_label(
                        "HTTPS-Only Mode is enabled. The site you are attempting to visit does not support HTTPS.",
                    );
                    ui_fail.https_continue_btn.set_visible(true);
                }

                ui_fail.stack.set_visible_child_name("https_error");
                return true;
            }

            // Subresource isolation: If the failure is for an auxiliary resource or subframe
            // while the main page has already committed or navigated to a different URL, do NOT
            // tear down the main document and display a full-page error interstitial.
            let is_main_document_load = failing_uri == tab_url
                || failing_uri == wv_uri
                || wv_uri.is_empty()
                || tab_url.is_empty();

            if !is_main_document_load {
                tracing::warn!(
                    "Subresource load failed for {failing_uri} ({error}) at stage {load_event:?}; main document {tab_url} preserved"
                );
                return false;
            }

            tb_fail.set_is_loading(false);
            page_fail.set_title("Connection Error");
            let mut tab = tab_c_fail.borrow_mut();
            tab.is_loading = false;
            tab.title = "Connection Error".to_string();
            drop(tab);

            ui_fail
                .error_label
                .set_label(&format!("Could not connect to:\n{failing_uri}\n\n{error}"));
            ui_fail.stack.set_visible_child_name("error");
            true
        });

        // 7. connect_load_failed_with_tls_errors (Safe warning page, NO silent bypass!)
        let ui_tls = ui_item.clone();
        let page_tls = page.clone();
        let tab_c_tls = tab_c.clone();

        webview.connect_load_failed_with_tls_errors(move |wv, failing_uri, _cert, errors| {
            crate::utils::net_diagnostics::record_tls_failure(failing_uri, errors);

            let tab_url = tab_c_tls.borrow().url.clone();
            let wv_uri = wv.uri().map(|u| u.to_string()).unwrap_or_default();

            // If a subresource has TLS errors, let WebKit drop the subresource without
            // taking down the entire main document unless the main document itself failed.
            let is_main_document = failing_uri == tab_url
                || failing_uri == wv_uri
                || wv_uri.is_empty()
                || tab_url.is_empty();

            if !is_main_document {
                tracing::warn!("Subresource TLS failure for {failing_uri} ({errors:?}); dropped");
                return false;
            }

            tracing::warn!("Main document TLS certificate failure for: {failing_uri}");
            page_tls.set_title("Privacy Warning");
            let mut tab = tab_c_tls.borrow_mut();
            tab.is_loading = false;
            tab.title = "Privacy Warning".to_string();
            ui_tls
                .tls_host_label
                .set_label(&format!("Host: {failing_uri}"));
            ui_tls.tls_detail_label.set_label(
                "Attackers might be trying to steal your information from this site.\n\
                 Shadhin Browser blocked this connection to protect your privacy.",
            );
            ui_tls.stack.set_visible_child_name("tls");
            true
        });

        // 8. connect_permission_request (Camera, Mic, Geo, Notification, Clipboard)
        let ui_perm = ui_item.clone();
        let storage_perm = self.storage.clone();
        let wv_perm = webview.clone();
        webview.connect_permission_request(move |_, req| {
            let (ptype, _) = identify_permission_request(req);
            let domain = wv_perm
                .uri()
                .and_then(|u| extract_host_from_uri(u.as_str()))
                .unwrap_or_else(|| "This site".to_string());

            let ui_perm_async = ui_perm.clone();
            let req_async = req.clone();
            let domain_async = domain.clone();

            storage_perm.query_async(
                move |db| {
                    let repo = PermissionRepository::new(db.conn());
                    repo.get_permission(&domain_async, ptype).unwrap_or(None)
                },
                move |stored_state| match stored_state {
                    Some(PermissionState::Allow) => {
                        info!("Auto-allowing {ptype:?} for {domain} (saved preference)");
                        req_async.allow();
                    }
                    Some(PermissionState::Block) => {
                        info!("Auto-blocking {ptype:?} for {domain} (saved preference)");
                        req_async.deny();
                    }
                    None | Some(PermissionState::Ask) => {
                        let (icon_name, p_name) = match ptype {
                            crate::browser::permissions::PermissionType::Camera => {
                                ("camera-web-symbolic", "camera")
                            }
                            crate::browser::permissions::PermissionType::Microphone => {
                                ("audio-input-microphone-symbolic", "microphone")
                            }
                            crate::browser::permissions::PermissionType::CameraAndMicrophone => {
                                ("camera-web-symbolic", "camera and microphone")
                            }
                            crate::browser::permissions::PermissionType::Geolocation => {
                                ("find-location-symbolic", "location")
                            }
                            crate::browser::permissions::PermissionType::Notifications => {
                                ("preferences-system-notifications-symbolic", "notifications")
                            }
                            crate::browser::permissions::PermissionType::Clipboard => {
                                ("edit-paste-symbolic", "clipboard")
                            }
                            crate::browser::permissions::PermissionType::DeviceInfo => {
                                ("computer-symbolic", "device information")
                            }
                            crate::browser::permissions::PermissionType::Other => {
                                ("dialog-question-symbolic", "device features")
                            }
                        };

                        ui_perm_async.permission_icon.set_icon_name(Some(icon_name));
                        ui_perm_async
                            .permission_label
                            .set_label(&format!("{domain} wants to access your {p_name}"));
                        *ui_perm_async.active_permission_request.borrow_mut() = Some(req_async);
                        ui_perm_async.permission_revealer.set_reveal_child(true);
                    }
                },
            );
            true
        });

        // 9. connect_web_process_terminated
        let ui_crash = ui_item.clone();
        webview.connect_web_process_terminated(move |_, reason| {
            tracing::error!("Web process terminated: {reason:?}");
            ui_crash
                .error_label
                .set_label("This web page crashed unexpectedly.\nClick Try Again to reload.");
            ui_crash.stack.set_visible_child_name("error");
        });

        // 10. connect_create (Window creation & Popup blocking policy)
        let nav_create = self.clone();
        webview.connect_create(move |_, action| {
            if let Some(req) = action.request()
                && let Some(uri) = req.uri()
            {
                let uri_str = uri.as_str();
                info!("Page requested new window for {uri_str}; redirecting to new tab");
                nav_create.create_tab(Some(uri_str), Some(tab_idx));
            }
            None
        });

        // 11. connect_context_menu
        let nav_cm = self.clone();
        webview.connect_context_menu(move |_, menu, hit_test| {
            if hit_test.context_is_link()
                && let Some(link_uri) = hit_test.link_uri()
            {
                let nav_item = nav_cm.clone();
                let uri_str = link_uri.to_string();
                let act_new_tab = gtk4::gio::SimpleAction::new("open-link-new-tab", None);
                act_new_tab.connect_activate(move |_, _| {
                    nav_item.create_tab(Some(&uri_str), None);
                });
                let item = webkit6::ContextMenuItem::from_gaction(
                    &act_new_tab,
                    "Open Link in New Tab",
                    None,
                );
                menu.append(&item);

                let item_save_link = webkit6::ContextMenuItem::from_stock_action(
                    webkit6::ContextMenuAction::DownloadLinkToDisk,
                );
                menu.append(&item_save_link);
            }

            if hit_test.context_is_image() {
                let item_save_img = webkit6::ContextMenuItem::from_stock_action(
                    webkit6::ContextMenuAction::DownloadImageToDisk,
                );
                menu.append(&item_save_img);
            }

            false
        });

        // 12. Fullscreen
        webview.connect_enter_fullscreen(|_| false);
        webview.connect_leave_fullscreen(|_| false);

        webview
    }

    pub fn execute_navigation(
        &self,
        tab_idx: usize,
        target: &NavigationTarget,
        page: &adw::TabPage,
    ) {
        match target {
            NavigationTarget::Url(raw_url) | NavigationTarget::Search(raw_url) => {
                let clean_url = crate::privacy::tracker::strip_tracking_parameters(raw_url);
                let tm = self.tab_manager.borrow();
                if let Some(tab_cell) = tm.get_tab(tab_idx) {
                    let tab_id = tab_cell.borrow().id;
                    self.https_manager
                        .borrow_mut()
                        .clear_upgrade_attempt(tab_id);

                    let mut tab = tab_cell.borrow_mut();
                    tab.url = clean_url.clone();
                    tab.title = "Loading...".to_string();
                    tab.is_loading = true;
                    tab.has_active_webview = true;
                    page.set_title("Loading...");
                    self.toolbar.address_bar().set_text(&clean_url);
                    self.toolbar
                        .address_bar()
                        .set_security_state(clean_url.starts_with("https://"));
                }
                drop(tm);

                let items = self.tab_items.borrow();
                if let Some(item) = items.get(tab_idx) {
                    let wv_opt = item.webview.borrow_mut();
                    let wv = if let Some(ref existing) = *wv_opt {
                        existing.clone()
                    } else {
                        drop(wv_opt);
                        self.setup_webview_for_tab(tab_idx, page)
                    };

                    item.stack.set_visible_child_name("web");
                    wv.load_uri(&clean_url);
                }

                self.sync_tabs();
            }
            NavigationTarget::Internal(page_id) => {
                let tm = self.tab_manager.borrow();
                if let Some(tab_cell) = tm.get_tab(tab_idx) {
                    let tab_id = tab_cell.borrow().id;
                    self.https_manager
                        .borrow_mut()
                        .clear_upgrade_attempt(tab_id);
                }
                drop(tm);

                let items = self.tab_items.borrow();
                let ui_item = items.get(tab_idx).cloned();
                drop(items);

                if page_id == "shadhin:bookmarks" || page_id == "about:bookmarks" {
                    let tm = self.tab_manager.borrow();
                    if let Some(tab_cell) = tm.get_tab(tab_idx) {
                        let mut tab = tab_cell.borrow_mut();
                        tab.url = "about:bookmarks".to_string();
                        tab.title = "Bookmarks".to_string();
                        tab.favicon = None;
                        tab.is_loading = false;
                        page.set_title("Bookmarks");
                        self.toolbar.address_bar().set_text("about:bookmarks");
                        self.toolbar.address_bar().set_security_state(true);
                    }
                    drop(tm);
                    if let Some(item) = ui_item {
                        item.bookmarks_panel.refresh("");
                        item.stack.set_visible_child_name("bookmarks");
                    }
                    self.sync_tabs();
                } else if page_id == "shadhin:history" || page_id == "about:history" {
                    let tm = self.tab_manager.borrow();
                    if let Some(tab_cell) = tm.get_tab(tab_idx) {
                        let mut tab = tab_cell.borrow_mut();
                        tab.url = "about:history".to_string();
                        tab.title = "History".to_string();
                        tab.favicon = None;
                        tab.is_loading = false;
                        page.set_title("History");
                        self.toolbar.address_bar().set_text("about:history");
                        self.toolbar.address_bar().set_security_state(true);
                    }
                    drop(tm);
                    if let Some(item) = ui_item {
                        item.history_panel.refresh("");
                        item.stack.set_visible_child_name("history");
                    }
                    self.sync_tabs();
                } else if page_id == "shadhin:downloads" || page_id == "about:downloads" {
                    let tm = self.tab_manager.borrow();
                    if let Some(tab_cell) = tm.get_tab(tab_idx) {
                        let mut tab = tab_cell.borrow_mut();
                        tab.url = "about:downloads".to_string();
                        tab.title = "Downloads".to_string();
                        tab.favicon = None;
                        tab.is_loading = false;
                        page.set_title("Downloads");
                        self.toolbar.address_bar().set_text("about:downloads");
                        self.toolbar.address_bar().set_security_state(true);
                    }
                    drop(tm);
                    if let Some(item) = ui_item {
                        item.downloads_panel.refresh("");
                        item.stack.set_visible_child_name("downloads");
                    }
                    self.sync_tabs();
                } else if page_id == "shadhin:newtab"
                    || page_id == "about:newtab"
                    || page_id == "about:blank"
                {
                    let tm = self.tab_manager.borrow();
                    if let Some(tab_cell) = tm.get_tab(tab_idx) {
                        let mut tab = tab_cell.borrow_mut();
                        tab.url = "about:newtab".to_string();
                        tab.title = "New Tab".to_string();
                        tab.favicon = None;
                        tab.is_loading = false;
                        page.set_title("New Tab");
                        self.toolbar.address_bar().set_text("");
                        self.toolbar.address_bar().set_security_state(true);
                    }
                    drop(tm);

                    if let Some(item) = ui_item {
                        item.stack.set_visible_child_name("newtab");
                    }
                    self.sync_tabs();
                }
            }
            NavigationTarget::Blocked { scheme, reason } => {
                info!("Navigation blocked for scheme {scheme}: {reason}");
                let items = self.tab_items.borrow();
                if let Some(item) = items.get(tab_idx) {
                    item.error_label.set_label(&format!(
                        "Navigation Blocked by Security Policy\n\n\
                         Scheme '{scheme}:' is not permitted.\n\n\
                         {reason}"
                    ));
                    item.stack.set_visible_child_name("error");
                    page.set_title("Navigation Blocked");
                }
            }
        }
    }

    pub fn sync_tabs(&self) {
        let tm = self.tab_manager.borrow();
        let tabs_info: Vec<TabDisplayInfo> = tm
            .tabs()
            .iter()
            .map(|t| {
                let b = t.borrow();
                TabDisplayInfo {
                    title: b.title.clone(),
                    favicon: b.favicon.clone(),
                    is_newtab: b.is_newtab(),
                    is_discarded: b.is_discarded,
                }
            })
            .collect();

        self.tab_strip.refresh(&tabs_info);

        let sel_idx = tm.selected_index();
        let tv_clone = self.tab_view.clone();
        self.sidebar
            .update_open_tabs(&tabs_info, sel_idx as i32, move |idx| {
                if idx >= 0 && idx < tv_clone.n_pages() {
                    let page = tv_clone.nth_page(idx);
                    tv_clone.set_selected_page(&page);
                }
            });

        if let Some(active_tab_cell) = tm.active_tab() {
            let active = active_tab_cell.borrow();
            if active.is_newtab() {
                self.toolbar.address_bar().set_text_if_unfocused("");
                self.toolbar.address_bar().set_security_state(true);
                self.toolbar.set_can_go_back(false);
                self.toolbar.set_can_go_forward(false);
                self.toolbar.set_is_loading(false);
                self.toolbar.address_bar().set_progress(0.0);
            } else {
                self.toolbar
                    .address_bar()
                    .set_text_if_unfocused(&active.url);
                self.toolbar
                    .address_bar()
                    .set_security_state(active.url.starts_with("https://"));
                self.toolbar.set_can_go_back(active.can_go_back);
                self.toolbar.set_can_go_forward(active.can_go_forward);
                self.toolbar.set_is_loading(active.is_loading);
                self.toolbar.address_bar().set_progress(active.progress);
            }

            let items = self.tab_items.borrow();
            if let Some(ui_item) = items.get(sel_idx)
                && let Some(ref wv) = *ui_item.webview.borrow()
            {
                self.find_bar.attach_webview(wv);
            }
        }
    }
}

impl ShadhinWindow {
    /// Applies an effective sidebar state to the split view, sidebar, and titlebar
    fn apply_sidebar_state(
        state: SidebarState,
        split_view: &adw::OverlaySplitView,
        sidebar: &Rc<ShadhinSidebar>,
        titlebar: &Rc<ShadhinTitlebar>,
        effective_cell: &Rc<RefCell<SidebarState>>,
    ) {
        *effective_cell.borrow_mut() = state;
        match state {
            SidebarState::Hidden => {
                split_view.set_show_sidebar(false);
                titlebar.update_sidebar_state(SidebarState::Hidden);
            }
            SidebarState::Compact => {
                split_view.set_show_sidebar(true);
                split_view.set_min_sidebar_width(56.0);
                split_view.set_max_sidebar_width(56.0);
                split_view.set_sidebar_width_fraction(0.04);
                sidebar.set_compact_mode(true);
                titlebar.update_sidebar_state(SidebarState::Compact);
            }
            SidebarState::Expanded => {
                split_view.set_show_sidebar(true);
                split_view.set_min_sidebar_width(200.0);
                split_view.set_max_sidebar_width(280.0);
                split_view.set_sidebar_width_fraction(0.18);
                sidebar.set_compact_mode(false);
                titlebar.update_sidebar_state(SidebarState::Expanded);
            }
        }
    }

    /// Toggles the sidebar between visible (Expanded or Compact) and Hidden, updating user preference
    #[allow(clippy::too_many_arguments)]
    fn toggle_sidebar_action(
        win_width: i32,
        saved_cell: &Rc<RefCell<SidebarState>>,
        cfg_cell: &Rc<RefCell<BrowserConfig>>,
        split_view: &adw::OverlaySplitView,
        sidebar: &Rc<ShadhinSidebar>,
        titlebar: &Rc<ShadhinTitlebar>,
        effective_cell: &Rc<RefCell<SidebarState>>,
    ) {
        let cur_eff = *effective_cell.borrow();
        let cur_saved = *saved_cell.borrow();
        let new_pref = match cur_eff {
            SidebarState::Hidden => {
                if cur_saved != SidebarState::Hidden {
                    cur_saved
                } else {
                    SidebarState::Expanded
                }
            }
            SidebarState::Expanded | SidebarState::Compact => SidebarState::Hidden,
        };

        *saved_cell.borrow_mut() = new_pref;
        cfg_cell.borrow_mut().sidebar_state = new_pref;
        let _ = cfg_cell.borrow().save();

        let effective = compute_effective_sidebar_state(win_width, new_pref);
        Self::apply_sidebar_state(effective, split_view, sidebar, titlebar, effective_cell);
    }

    /// Sets an explicit sidebar state chosen by the user from the titlebar menu
    #[allow(clippy::too_many_arguments)]
    fn set_explicit_sidebar_state(
        new_state: SidebarState,
        win_width: i32,
        saved_cell: &Rc<RefCell<SidebarState>>,
        cfg_cell: &Rc<RefCell<BrowserConfig>>,
        split_view: &adw::OverlaySplitView,
        sidebar: &Rc<ShadhinSidebar>,
        titlebar: &Rc<ShadhinTitlebar>,
        effective_cell: &Rc<RefCell<SidebarState>>,
    ) {
        *saved_cell.borrow_mut() = new_state;
        cfg_cell.borrow_mut().sidebar_state = new_state;
        let _ = cfg_cell.borrow().save();

        let effective = compute_effective_sidebar_state(win_width, new_state);
        Self::apply_sidebar_state(effective, split_view, sidebar, titlebar, effective_cell);
    }

    /// Sets up cross-component UI signals
    fn setup_signals(&self) {
        // 1. Sidebar Toggle Button in Titlebar and Toolbar
        let win_w1 = self.window.clone();
        let saved_cell1 = self.saved_sidebar_state.clone();
        let cfg_cell1 = self.config.clone();
        let sv1 = self.split_view.clone();
        let sb1 = self.sidebar.clone();
        let tb_tb1 = self.titlebar.clone();
        let eff1 = self.effective_sidebar_state.clone();

        self.titlebar.connect_sidebar_toggle(move || {
            Self::toggle_sidebar_action(
                win_w1.width(),
                &saved_cell1,
                &cfg_cell1,
                &sv1,
                &sb1,
                &tb_tb1,
                &eff1,
            );
        });

        let win_w_st = self.window.clone();
        let saved_cell_st = self.saved_sidebar_state.clone();
        let cfg_cell_st = self.config.clone();
        let sv_st = self.split_view.clone();
        let sb_st = self.sidebar.clone();
        let tb_tb_st = self.titlebar.clone();
        let eff_st = self.effective_sidebar_state.clone();

        self.titlebar.connect_sidebar_state_selected(move |chosen| {
            Self::set_explicit_sidebar_state(
                chosen,
                win_w_st.width(),
                &saved_cell_st,
                &cfg_cell_st,
                &sv_st,
                &sb_st,
                &tb_tb_st,
                &eff_st,
            );
        });

        let win_w2 = self.window.clone();
        let saved_cell2 = self.saved_sidebar_state.clone();
        let cfg_cell2 = self.config.clone();
        let sv2 = self.split_view.clone();
        let sb2 = self.sidebar.clone();
        let tb_tb2 = self.titlebar.clone();
        let eff2 = self.effective_sidebar_state.clone();

        self.toolbar.connect_sidebar_toggle(move || {
            Self::toggle_sidebar_action(
                win_w2.width(),
                &saved_cell2,
                &cfg_cell2,
                &sv2,
                &sb2,
                &tb_tb2,
                &eff2,
            );
        });

        // Bookmarks Bar item click navigation
        let nav_bb = self.nav_ctx.clone();
        let tv_bb = self.tab_view.clone();
        let cfg_bb = self.config.clone();

        self.bookmarks_bar.connect_navigate(move |url| {
            if let Some(page) = tv_bb.selected_page() {
                let idx = tv_bb.page_position(&page) as usize;
                let target = classify_input(url, &cfg_bb.borrow().search_engine_url);
                nav_bb.execute_navigation(idx, &target, &page);
            }
        });

        // 2. New Tab Button in Titlebar Tab Strip
        let nav_new = self.nav_ctx.clone();
        self.tab_strip.connect_new_tab(move |opt_url| {
            info!("New Tab requested (URL: {opt_url:?})");
            nav_new.create_tab(opt_url.as_deref(), None);
        });

        // 3. Tab Context Menu Actions: Duplicate Tab
        let tm_dup = self.tab_manager.clone();
        let tv_dup = self.tab_view.clone();
        let ts_dup = self.tab_strip.clone();
        let sb_dup = self.sidebar.clone();
        let tb_dup = self.toolbar.clone();
        let self_dup = self.window.clone();
        let _ = self_dup;

        self.tab_strip.connect_duplicate_tab(move |idx| {
            if idx >= 0 {
                info!("Duplicating tab {idx}");
                let mut tm = tm_dup.borrow_mut();
                if let Some((_new_idx, _tab)) = tm.duplicate_tab(idx as usize) {
                    drop(tm);
                    Self::sync_tabs_external(&tm_dup, &ts_dup, &sb_dup, &tv_dup, &tb_dup);
                }
            }
        });

        // 4. Tab Context Menu Actions: Close Other Tabs
        let tm_co = self.tab_manager.clone();
        let tv_co = self.tab_view.clone();
        let ts_co = self.tab_strip.clone();
        let sb_co = self.sidebar.clone();
        let tb_co = self.toolbar.clone();

        self.tab_strip.connect_close_other_tabs(move |idx| {
            if idx >= 0 {
                info!("Closing all tabs except tab {idx}");
                let mut tm = tm_co.borrow_mut();
                tm.close_other_tabs(idx as usize);
                drop(tm);
                while tv_co.n_pages() > 1 {
                    let page = tv_co.nth_page(0);
                    tv_co.close_page_finish(&page, true);
                }
                Self::sync_tabs_external(&tm_co, &ts_co, &sb_co, &tv_co, &tb_co);
            }
        });

        // 5. Tab View Page Changed notification
        let tm_change = self.tab_manager.clone();
        let tb_change = self.toolbar.clone();
        let ts_change = self.tab_strip.clone();
        let sb_change = self.sidebar.clone();
        let tv_change = self.tab_view.clone();
        let fb_change = self.find_bar.clone();
        let items_change = self.tab_items.clone();
        let nav_change = self.nav_ctx.clone();

        self.tab_view.connect_selected_page_notify(move |tv| {
            if let Some(page) = tv.selected_page() {
                let idx = tv.page_position(&page) as usize;
                let mut tm = tm_change.borrow_mut();
                tm.switch_tab(idx);

                // Low-RAM tab discarding check: discard idle background tabs if > threshold
                let discarded =
                    tm.discard_idle_tabs(TAB_DISCARD_THRESHOLD, Duration::from_secs(300));
                for d_idx in discarded {
                    let items = items_change.borrow();
                    if let Some(item) = items.get(d_idx) {
                        let old_wv = item.webview.borrow_mut().take();
                        if let Some(ref wv) = old_wv {
                            item.stack.remove(wv);
                            info!("Removed active WebView for discarded tab {d_idx} to free WebProcess RAM");
                        }
                    }
                }
                drop(tm);

                // Check if activated tab was discarded and needs WebView reconstruction
                let (needs_restore, target_url) = {
                    let items = items_change.borrow();
                    let needs = items
                        .get(idx)
                        .map(|item| item.webview.borrow().is_none())
                        .unwrap_or(false);
                    let tm = tm_change.borrow();
                    let url = tm
                        .get_tab(idx)
                        .map(|t| t.borrow().url.clone())
                        .unwrap_or_default();
                    (needs, url)
                };

                if needs_restore
                    && (target_url.starts_with("http://") || target_url.starts_with("https://"))
                {
                    info!("Restoring discarded tab {idx} and reloading URL: {target_url}");
                    let webview = nav_change.setup_webview_for_tab(idx, &page);
                    webview.load_uri(&target_url);
                }

                Self::sync_tabs_external(
                    &tm_change, &ts_change, &sb_change, &tv_change, &tb_change,
                );

                // Attach FindBar to newly selected tab webview
                let items = items_change.borrow();
                if let Some(item) = items.get(idx)
                    && let Some(ref wv) = *item.webview.borrow()
                {
                    fb_change.attach_webview(wv);
                }
            }
        });

        // 6. Tab Close Page handler
        let tm_close = self.tab_manager.clone();
        let items_close = self.tab_items.clone();
        let ts_close = self.tab_strip.clone();
        let sb_close = self.sidebar.clone();
        let tb_close = self.toolbar.clone();
        let tv_close = self.tab_view.clone();
        let https_mgr_close = self.https_manager.clone();

        self.tab_view.connect_close_page(move |tv, page| {
            let idx = tv.page_position(page) as usize;
            info!("Closing tab at index {idx}");
            let tab_id = tm_close.borrow().get_tab(idx).map(|t| t.borrow().id);
            if let Some(id) = tab_id {
                https_mgr_close.borrow_mut().clear_upgrade_attempt(id);
            }

            let mut tm = tm_close.borrow_mut();
            let _ = tm.close_tab(idx);
            drop(tm);

            let mut items = items_close.borrow_mut();
            if idx < items.len() {
                items.remove(idx);
            }
            drop(items);

            tv.close_page_finish(page, true);
            Self::sync_tabs_external(&tm_close, &ts_close, &sb_close, &tv_close, &tb_close);
            glib::Propagation::Proceed
        });

        // 7. Address Bar Submission
        let nav_addr = self.nav_ctx.clone();
        let tv_addr = self.tab_view.clone();
        let cfg_addr = self.config.clone();

        self.toolbar.address_bar().connect_activate(move |input| {
            if let Some(page) = tv_addr.selected_page() {
                let idx = tv_addr.page_position(&page) as usize;
                let target = classify_input(input, &cfg_addr.borrow().search_engine_url);
                nav_addr.execute_navigation(idx, &target, &page);
            }
        });

        // 8. Toolbar Navigation: Back, Forward, Reload / Stop
        let items_nav = self.tab_items.clone();
        let tv_nav = self.tab_view.clone();

        let items_b = items_nav.clone();
        let tv_b = tv_nav.clone();
        self.toolbar.connect_back(move || {
            if let Some(page) = tv_b.selected_page() {
                let idx = tv_b.page_position(&page) as usize;
                let items = items_b.borrow();
                if let Some(item) = items.get(idx)
                    && let Some(ref wv) = *item.webview.borrow()
                {
                    wv.go_back();
                }
            }
        });

        let items_f = items_nav.clone();
        let tv_f = tv_nav.clone();
        self.toolbar.connect_forward(move || {
            if let Some(page) = tv_f.selected_page() {
                let idx = tv_f.page_position(&page) as usize;
                let items = items_f.borrow();
                if let Some(item) = items.get(idx)
                    && let Some(ref wv) = *item.webview.borrow()
                {
                    wv.go_forward();
                }
            }
        });

        let items_r = items_nav.clone();
        let tv_r = tv_nav.clone();
        self.toolbar.connect_reload(move |is_loading| {
            if let Some(page) = tv_r.selected_page() {
                let idx = tv_r.page_position(&page) as usize;
                let items = items_r.borrow();
                if let Some(item) = items.get(idx)
                    && let Some(ref wv) = *item.webview.borrow()
                {
                    if is_loading {
                        wv.stop_loading();
                    } else {
                        wv.reload();
                    }
                }
            }
        });

        // 9. Sidebar Navigation Items
        let tm_sb = self.tab_manager.clone();
        let items_sb = self.tab_items.clone();
        let tv_sb = self.tab_view.clone();
        let tb_sb = self.toolbar.clone();
        let win_sb = self.window.clone();
        let cfg_sb = self.config.clone();
        let storage_sb = self.storage.clone();
        let session_sb = self.nav_ctx.session.clone();

        self.sidebar.connect_page_selected(move |page_id| {
            info!("Sidebar item chosen: {page_id}");
            match page_id {
                "home" => {
                    if let Some(page) = tv_sb.selected_page() {
                        let idx = tv_sb.page_position(&page) as usize;
                        let tm = tm_sb.borrow();
                        if let Some(tab_cell) = tm.get_tab(idx) {
                            let mut tab = tab_cell.borrow_mut();
                            tab.url = "about:newtab".to_string();
                            tab.title = "New Tab".to_string();
                            tab.favicon = None;
                        }
                        page.set_title("New Tab");
                        tb_sb.address_bar().set_text("");
                        let items = items_sb.borrow();
                        if let Some(item) = items.get(idx) {
                            item.stack.set_visible_child_name("newtab");
                        }
                    }
                }
                "bookmarks" => {
                    if let Some(page) = tv_sb.selected_page() {
                        let idx = tv_sb.page_position(&page) as usize;
                        let tm = tm_sb.borrow();
                        if let Some(tab_cell) = tm.get_tab(idx) {
                            let mut tab = tab_cell.borrow_mut();
                            tab.url = "about:bookmarks".to_string();
                            tab.title = "Bookmarks".to_string();
                            tab.favicon = None;
                        }
                        page.set_title("Bookmarks");
                        tb_sb.address_bar().set_text("about:bookmarks");
                        let items = items_sb.borrow();
                        if let Some(item) = items.get(idx) {
                            item.bookmarks_panel.refresh("");
                            item.stack.set_visible_child_name("bookmarks");
                        }
                    }
                }
                "history" => {
                    if let Some(page) = tv_sb.selected_page() {
                        let idx = tv_sb.page_position(&page) as usize;
                        let tm = tm_sb.borrow();
                        if let Some(tab_cell) = tm.get_tab(idx) {
                            let mut tab = tab_cell.borrow_mut();
                            tab.url = "about:history".to_string();
                            tab.title = "History".to_string();
                            tab.favicon = None;
                        }
                        page.set_title("History");
                        tb_sb.address_bar().set_text("about:history");
                        let items = items_sb.borrow();
                        if let Some(item) = items.get(idx) {
                            item.history_panel.refresh("");
                            item.stack.set_visible_child_name("history");
                        }
                    }
                }
                "downloads" => {
                    if let Some(page) = tv_sb.selected_page() {
                        let idx = tv_sb.page_position(&page) as usize;
                        let tm = tm_sb.borrow();
                        if let Some(tab_cell) = tm.get_tab(idx) {
                            let mut tab = tab_cell.borrow_mut();
                            tab.url = "about:downloads".to_string();
                            tab.title = "Downloads".to_string();
                            tab.favicon = None;
                        }
                        page.set_title("Downloads");
                        tb_sb.address_bar().set_text("about:downloads");
                        let items = items_sb.borrow();
                        if let Some(item) = items.get(idx) {
                            item.downloads_panel.refresh("");
                            item.stack.set_visible_child_name("downloads");
                        }
                    }
                }
                "settings" => {
                    SettingsWindow::present(
                        Some(&win_sb),
                        cfg_sb.clone(),
                        storage_sb.clone(),
                        Some(session_sb.borrow().clone()),
                    );
                }
                _ => {}
            }
        });

        // 10. Workspace Switcher
        let tm_ws = self.tab_manager.clone();
        let cur_ws = self.current_workspace.clone();
        let session_ws = self.session_mgr.clone();
        let active_sess_ws = self.active_network_session.clone();
        let dl_mgr_ws = self.download_mgr.clone();
        let persistence_allowed_ws = self.persistence_allowed;

        self.sidebar.set_on_workspace_selected(move |target_ws| {
            let prev_ws = cur_ws.borrow().clone();
            if prev_ws == target_ws {
                return;
            }

            // Save open tabs of current workspace
            let tabs: Vec<(String, String, bool)> = tm_ws
                .borrow()
                .tabs()
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let b = t.borrow();
                    (
                        b.url.clone(),
                        b.title.clone(),
                        i == tm_ws.borrow().selected_index(),
                    )
                })
                .collect();

            session_ws
                .borrow()
                .save_workspace_tabs(&prev_ws, tabs, persistence_allowed_ws);
            *cur_ws.borrow_mut() = target_ws.to_string();

            if persistence_allowed_ws {
                let new_session = session_ws
                    .borrow_mut()
                    .get_or_create_workspace_session(target_ws);
                *active_sess_ws.borrow_mut() = new_session.clone();
                dl_mgr_ws.attach_session(&new_session, true);
                apply_cookie_policy(
                    &new_session,
                    crate::privacy::cookies::CookiePolicy::BlockThirdParty,
                );
                apply_itp(&new_session, true);
            }

            info!("Switched to workspace '{target_ws}'");
        });

        // 11. Create New Workspace handler
        let sb_new_ws = self.sidebar.clone();
        let storage_new_ws = self.storage.clone();
        let sm_new_ws = self.session_mgr.clone();
        let win_new_ws = self.window.clone();

        self.sidebar.set_on_create_workspace(move || {
            let entry = gtk4::Entry::builder()
                .placeholder_text("Workspace name (e.g. Research)")
                .build();

            let dialog = adw::MessageDialog::builder()
                .heading("New Workspace")
                .body("Enter a name for the new isolated workspace:")
                .transient_for(&win_new_ws)
                .build();

            dialog.set_extra_child(Some(&entry));
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("create", "Create");
            dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);

            let st = storage_new_ws.clone();
            let sb = sb_new_ws.clone();
            let sm_click = sm_new_ws.clone();

            dialog.connect_response(None, move |d, resp| {
                if resp == "create" {
                    let name = entry.text().trim().to_string();
                    if !name.is_empty() {
                        let ws_id = name.to_lowercase().replace(' ', "-");
                        let n = name.clone();
                        let id_clone = ws_id.clone();
                        st.dispatch(move |db| {
                            let repo = WorkspaceRepository::new(db.conn());
                            let _ = repo.create(&id_clone, &n, "#4ade80");
                        });
                        sm_click
                            .borrow_mut()
                            .get_or_create_workspace_session(&ws_id);
                        sb.add_workspace_item(&ws_id, &name);
                        info!("Created new workspace: '{name}' (id: '{ws_id}')");
                    }
                }
                d.destroy();
            });
            dialog.present();
        });

        // 12. Clean Shutdown on Close Request
        let cur_ws_close = self.current_workspace.clone();
        let tm_close = self.tab_manager.clone();
        let sm_close = self.session_mgr.clone();
        let storage_close = self.storage.clone();

        self.window.connect_close_request(move |_| {
            let ws = cur_ws_close.borrow().clone();
            let tabs: Vec<(String, String, bool)> = tm_close
                .borrow()
                .tabs()
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let b = t.borrow();
                    (
                        b.url.clone(),
                        b.title.clone(),
                        i == tm_close.borrow().selected_index(),
                    )
                })
                .collect();
            sm_close.borrow().save_workspace_tabs(&ws, tabs, true);
            sm_close.borrow().on_clean_shutdown();
            storage_close.optimize();
            gtk4::glib::Propagation::Proceed
        });

        // 13. Privacy Shield Popover Callbacks
        let app_pv = self.window.application().and_downcast::<adw::Application>();
        self.toolbar
            .privacy_popover()
            .set_on_private_window(move || {
                if let Some(ref app) = app_pv {
                    ShadhinWindow::new_private(app).present();
                }
            });

        let win_cd = self.window.clone();
        let storage_cd = self.storage.clone();
        let session_cd = self.nav_ctx.session.clone();
        let perm_cd = self.persistence_allowed;
        self.toolbar.privacy_popover().set_on_clear_data(move || {
            crate::ui::clear_data::show_clear_browsing_data_dialog(
                &win_cd,
                storage_cd.clone(),
                session_cd.borrow().clone(),
                perm_cd,
            );
        });

        let win_st = self.window.clone();
        let cfg_st = self.config.clone();
        let storage_st = self.storage.clone();
        let session_st = self.nav_ctx.session.clone();
        self.toolbar
            .privacy_popover()
            .set_on_open_settings(move || {
                SettingsWindow::present(
                    Some(&win_st),
                    cfg_st.clone(),
                    storage_st.clone(),
                    Some(session_st.borrow().clone()),
                );
            });

        let nav_ts = self.nav_ctx.clone();
        self.toolbar
            .privacy_popover()
            .set_on_toggle_site(move |domain, enabled| {
                let d = domain.to_string();
                let perm = nav_ts.persistence_allowed;
                nav_ts.storage.dispatch(move |db| {
                    let repo = SiteExceptionRepository::new(db.conn());
                    let _ = repo.set_exception(&d, enabled, enabled, enabled, perm);
                });
            });

        let nav_tm = self.nav_ctx.clone();
        self.toolbar
            .privacy_popover()
            .set_on_toggle_master(move |active| {
                let tv = &nav_tm.tab_view;
                if let Some(page) = tv.selected_page() {
                    let idx = tv.page_position(&page) as usize;
                    let items = nav_tm.tab_items.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                        && let Some(ucm) = wv.user_content_manager()
                    {
                        let cfg = nav_tm.config.borrow();
                        nav_tm.filter_manager.apply_filters(
                            &ucm,
                            active,
                            cfg.privacy.block_ads,
                            cfg.privacy.block_trackers,
                        );
                    }
                }
            });
    }

    /// Sets up complete keyboard shortcuts using GTK4 event controllers
    fn setup_shortcuts(&self) {
        let key_controller = gtk4::EventControllerKey::new();

        let split_key = self.split_view.clone();
        let tb_key = self.toolbar.clone();
        let tv_key = self.tab_view.clone();
        let ts_key = self.tab_strip.clone();
        let tm_key = self.tab_manager.clone();
        let items_key = self.tab_items.clone();
        let fb_key = self.find_bar.clone();
        let win_self = self.window.clone();
        let cfg_key = self.config.clone();
        let storage_key = self.storage.clone();
        let sb_key = self.sidebar.clone();
        let bb_key = self.bookmarks_bar.clone();
        let saved_key = self.saved_sidebar_state.clone();
        let eff_key = self.effective_sidebar_state.clone();
        let titlebar_key = self.titlebar.clone();
        let nav_key = self.nav_ctx.clone();
        let session_key = self.nav_ctx.session.clone();
        let perm_key = self.persistence_allowed;

        key_controller.connect_key_pressed(move |_, key, _, modifier| {
            let ctrl = modifier.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            let alt = modifier.contains(gtk4::gdk::ModifierType::ALT_MASK);
            let shift = modifier.contains(gtk4::gdk::ModifierType::SHIFT_MASK);

            // Esc: Close Find in Page
            if key == Key::Escape && fb_key.is_visible() {
                fb_key.close();
                return glib::Propagation::Stop;
            }

            // Ctrl+F: Find in Page
            if ctrl && key == Key::f {
                fb_key.show_and_focus();
                return glib::Propagation::Stop;
            }

            // Ctrl+T: New Tab
            if ctrl && !shift && key == Key::t {
                info!("Shortcut: Ctrl+T New Tab");
                ts_key.trigger_new_tab(None);
                return glib::Propagation::Stop;
            }

            // Ctrl+W: Close active tab
            if ctrl && !shift && key == Key::w {
                if let Some(page) = tv_key.selected_page() {
                    tv_key.close_page(&page);
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+T: Reopen closed tab
            if ctrl && shift && key == Key::T {
                info!("Shortcut: Ctrl+Shift+T Reopen Closed Tab");
                let mut tm = tm_key.borrow_mut();
                if let Some(info) = tm.pop_closed_tab() {
                    drop(tm);
                    info!("Reopening closed tab: {}", info.url);
                    ts_key.trigger_new_tab(Some(info.url));
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+L or F6: Focus address bar
            if (ctrl && key == Key::l) || key == Key::F6 {
                tb_key.address_bar().focus();
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+R: Hard reload (bypass cache)
            if ctrl && shift && key == Key::R {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        wv.reload_bypass_cache();
                        info!("Hard reload initiated for active tab");
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+R or F5: Reload active tab
            if (ctrl && key == Key::r) || key == Key::F5 {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        wv.reload();
                    }
                }
                return glib::Propagation::Stop;
            }

            // Alt+Left: Back
            if alt && key == Key::Left {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        wv.go_back();
                    }
                }
                return glib::Propagation::Stop;
            }

            // Alt+Right: Forward
            if alt && key == Key::Right {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        wv.go_forward();
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Tab: Next tab
            if ctrl && !shift && key == Key::Tab {
                let n = tv_key.n_pages();
                if n > 1
                    && let Some(page) = tv_key.selected_page()
                {
                    let cur = tv_key.page_position(&page);
                    let next = (cur + 1) % n;
                    let next_page = tv_key.nth_page(next);
                    tv_key.set_selected_page(&next_page);
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+Tab: Previous tab
            if ctrl && shift && key == Key::ISO_Left_Tab {
                let n = tv_key.n_pages();
                if n > 1
                    && let Some(page) = tv_key.selected_page()
                {
                    let cur = tv_key.page_position(&page);
                    let prev = if cur == 0 { n - 1 } else { cur - 1 };
                    let prev_page = tv_key.nth_page(prev);
                    tv_key.set_selected_page(&prev_page);
                }
                return glib::Propagation::Stop;
            }

            // Alt+1..9: Jump to Tab 1 to 9
            if alt {
                let digit = match key {
                    Key::_1 => Some(0),
                    Key::_2 => Some(1),
                    Key::_3 => Some(2),
                    Key::_4 => Some(3),
                    Key::_5 => Some(4),
                    Key::_6 => Some(5),
                    Key::_7 => Some(6),
                    Key::_8 => Some(7),
                    Key::_9 => Some(8),
                    _ => None,
                };
                if let Some(target_idx) = digit
                    && target_idx < tv_key.n_pages()
                {
                    let page = tv_key.nth_page(target_idx);
                    tv_key.set_selected_page(&page);
                    return glib::Propagation::Stop;
                }
            }

            // Ctrl+Plus / Ctrl+=: Zoom in
            if ctrl && (key == Key::plus || key == Key::equal) {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        let new_zoom = (wv.zoom_level() + 0.1).min(5.0);
                        wv.set_zoom_level(new_zoom);
                        info!("Zoom level increased to: {new_zoom:.1}");
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Minus: Zoom out
            if ctrl && key == Key::minus {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        let new_zoom = (wv.zoom_level() - 0.1).max(0.25);
                        wv.set_zoom_level(new_zoom);
                        info!("Zoom level decreased to: {new_zoom:.1}");
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+0: Reset zoom
            if ctrl && key == Key::_0 {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx)
                        && let Some(ref wv) = *item.webview.borrow()
                    {
                        wv.set_zoom_level(1.0);
                        info!("Zoom level reset to 1.0");
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+B: Toggle Bookmarks Bar
            if ctrl && shift && key == Key::B {
                bb_key.toggle_visibility();
                return glib::Propagation::Stop;
            }

            // F9 or Ctrl+B: Toggle Sidebar
            if key == Key::F9 || (ctrl && !shift && key == Key::b) {
                Self::toggle_sidebar_action(
                    win_self.width(),
                    &saved_key,
                    &cfg_key,
                    &split_key,
                    &sb_key,
                    &titlebar_key,
                    &eff_key,
                );
                return glib::Propagation::Stop;
            }

            // Ctrl+,: Preferences / Settings Window
            if ctrl && key == Key::comma {
                SettingsWindow::present(
                    Some(&win_self),
                    cfg_key.clone(),
                    storage_key.clone(),
                    Some(session_key.borrow().clone()),
                );
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+N: New Private Window
            if ctrl && shift && key == Key::N {
                if let Some(app) = win_self.application().and_downcast::<adw::Application>() {
                    let priv_win = ShadhinWindow::new_private(&app);
                    priv_win.present();
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+Delete / KP_Delete: Clear Browsing Data Dialog
            if ctrl && shift && (key == Key::Delete || key == Key::KP_Delete) {
                crate::ui::clear_data::show_clear_browsing_data_dialog(
                    &win_self,
                    storage_key.clone(),
                    session_key.borrow().clone(),
                    perm_key,
                );
                return glib::Propagation::Stop;
            }

            // Ctrl+D: Bookmark active tab
            if ctrl && !shift && key == Key::d {
                tb_key.address_bar().trigger_bookmark();
                return glib::Propagation::Stop;
            }

            // Alt+Home: Navigate to homepage
            if alt && key == Key::Home {
                let home_url = cfg_key.borrow().homepage.clone();
                let target = classify_input(&home_url, &cfg_key.borrow().search_engine_url);
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    nav_key.execute_navigation(idx, &target, &page);
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+O: Open Bookmarks panel
            if ctrl && shift && key == Key::O {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx) {
                        item.bookmarks_panel.refresh("");
                        item.stack.set_visible_child_name("bookmarks");
                        tb_key.address_bar().set_text("about:bookmarks");
                        tb_key.address_bar().set_security_state(true);
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+H: Open History panel
            if ctrl && !shift && key == Key::h {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx) {
                        item.history_panel.refresh("");
                        item.stack.set_visible_child_name("history");
                        tb_key.address_bar().set_text("about:history");
                        tb_key.address_bar().set_security_state(true);
                    }
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+J: Open Downloads panel
            if ctrl && !shift && key == Key::j {
                if let Some(page) = tv_key.selected_page() {
                    let idx = tv_key.page_position(&page) as usize;
                    let items = items_key.borrow();
                    if let Some(item) = items.get(idx) {
                        item.downloads_panel.refresh("");
                        item.stack.set_visible_child_name("downloads");
                        tb_key.address_bar().set_text("about:downloads");
                        tb_key.address_bar().set_security_state(true);
                    }
                }
                return glib::Propagation::Stop;
            }

            // F11: Fullscreen toggle
            if key == Key::F11 {
                if win_self.is_fullscreen() {
                    win_self.unfullscreen();
                } else {
                    win_self.fullscreen();
                }
                return glib::Propagation::Stop;
            }

            // Ctrl+Shift+N: Private Window stub
            if ctrl && shift && key == Key::N {
                let toast = adw::Toast::new(
                    "Private browsing with isolated ephemeral storage arrives in Phase 5",
                );
                toast.set_timeout(3);
                info!("Ctrl+Shift+N pressed (DEV-ONLY: Phase 5)");
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });

        self.window.add_controller(key_controller);
    }

    /// Presents the window on the display
    pub fn present(&self) {
        self.window.present();
    }
}
