Name:           shadhin-browser
Version:        0.1.0
Release:        1%{?dist}
Summary:        Fast, private, and open native Linux web browser

License:        GPL-3.0-or-later
URL:            https://github.com/placeholder-user/shadhin-browser
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.80
BuildRequires:  cargo
BuildRequires:  pkgconfig(gtk4) >= 4.10.0
BuildRequires:  pkgconfig(libadwaita-1) >= 1.4.0
BuildRequires:  pkgconfig(webkitgtk-6.0) >= 2.42.0
BuildRequires:  pkgconfig(sqlite3) >= 3.35.0
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gtk4 >= 4.10.0
Requires:       libadwaita >= 1.4.0
Requires:       webkitgtk6.0 >= 2.42.0
Requires:       sqlite-libs >= 3.35.0
Requires:       bubblewrap
Requires:       hicolor-icon-theme

Recommends:     xdg-utils
Recommends:     google-noto-sans-bengali-fonts

%description
Shadhin Browser (স্বাধীন ব্রাউজার) is a fast, privacy-respecting native
Linux web browser built with Rust, GTK4, Libadwaita, and WebKitGTK 6.0.

Key Features:
- Native Wayland and X11 rendering via GTK4 and Libadwaita
- Multi-process WebKitGTK 6.0 engine with process isolation
- Native content filtering with bundled EasyList and EasyPrivacy rules
- Isolated workspaces with dedicated cookie and cache compartmentalization
- Telemetry-free architecture with strict 0700/0600 file permissions

%prep
%autosetup -n %{name}-%{version}

%build
cargo build --release --locked

%install
# Install binary
install -D -p -m 0755 target/release/shadhin-browser %{buildroot}%{_bindir}/shadhin-browser

# Install desktop file
install -D -p -m 0644 data/com.shadhin.browser.desktop %{buildroot}%{_datadir}/applications/com.shadhin.browser.desktop

# Install AppStream metadata
install -D -p -m 0644 data/com.shadhin.browser.metainfo.xml %{buildroot}%{_metainfodir}/com.shadhin.browser.metainfo.xml

# Install SVG icons
install -D -p -m 0644 data/icons/hicolor/scalable/apps/com.shadhin.browser.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/com.shadhin.browser.svg
install -D -p -m 0644 data/icons/hicolor/symbolic/apps/com.shadhin.browser-symbolic.svg %{buildroot}%{_datadir}/icons/hicolor/symbolic/apps/com.shadhin.browser-symbolic.svg

# Install raster icons
for size in 48 64 128 256 512; do
    install -D -p -m 0644 data/icons/hicolor/${size}x${size}/apps/com.shadhin.browser.png \
        %{buildroot}%{_datadir}/icons/hicolor/${size}x${size}/apps/com.shadhin.browser.png
done

# Install man page
install -D -p -m 0644 packaging/deb/usr/share/man/man1/shadhin-browser.1 %{buildroot}%{_mandir}/man1/shadhin-browser.1

%check
desktop-file-validate %{buildroot}%{_datadir}/applications/com.shadhin.browser.desktop
appstream-util validate-relax --nonet %{buildroot}%{_metainfodir}/com.shadhin.browser.metainfo.xml

%files
%license LICENSE THIRD_PARTY_LICENSES.md
%doc README.md CHANGELOG.md docs/
%{_bindir}/shadhin-browser
%{_datadir}/applications/com.shadhin.browser.desktop
%{_metainfodir}/com.shadhin.browser.metainfo.xml
%{_datadir}/icons/hicolor/*/apps/com.shadhin.browser.*
%{_mandir}/man1/shadhin-browser.1*

%changelog
* Sun Nov 03 2026 Shadhin Browser Contributors <placeholder@example.com> - 0.1.0-1
- Initial release of Shadhin Browser 0.1.0
