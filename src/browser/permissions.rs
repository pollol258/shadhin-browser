//! Site permission requests (camera, microphone, geolocation, notifications)
//!
//! Handles WebKitGTK 6.0 permission requests with persistent and ephemeral decision stores.

use rusqlite::{Connection, Result as SqlResult, params};
use std::fmt;
use tracing::info;
use webkit6::prelude::*;
use webkit6::{
    ClipboardPermissionRequest, DeviceInfoPermissionRequest, GeolocationPermissionRequest,
    NotificationPermissionRequest, PermissionRequest, UserMediaPermissionRequest,
};

/// Category of site permission requested
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionType {
    Geolocation,
    Notifications,
    Camera,
    Microphone,
    CameraAndMicrophone,
    Clipboard,
    DeviceInfo,
    Other,
}

impl PermissionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Geolocation => "geolocation",
            Self::Notifications => "notifications",
            Self::Camera => "camera",
            Self::Microphone => "microphone",
            Self::CameraAndMicrophone => "camera_and_microphone",
            Self::Clipboard => "clipboard",
            Self::DeviceInfo => "device_info",
            Self::Other => "other",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "geolocation" => Self::Geolocation,
            "notifications" => Self::Notifications,
            "camera" => Self::Camera,
            "microphone" => Self::Microphone,
            "camera_and_microphone" => Self::CameraAndMicrophone,
            "clipboard" => Self::Clipboard,
            "device_info" => Self::DeviceInfo,
            _ => Self::Other,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Geolocation => "Location Access",
            Self::Notifications => "Notifications",
            Self::Camera => "Camera Access",
            Self::Microphone => "Microphone Access",
            Self::CameraAndMicrophone => "Camera and Microphone",
            Self::Clipboard => "Clipboard Access",
            Self::DeviceInfo => "Device Information",
            Self::Other => "Hardware Access",
        }
    }

    pub fn icon_name(&self) -> &'static str {
        match self {
            Self::Geolocation => "find-location-symbolic",
            Self::Notifications => "preferences-system-notifications-symbolic",
            Self::Camera => "camera-web-symbolic",
            Self::Microphone => "audio-input-microphone-symbolic",
            Self::CameraAndMicrophone => "camera-web-symbolic",
            Self::Clipboard => "edit-paste-symbolic",
            Self::DeviceInfo => "computer-symbolic",
            Self::Other => "dialog-question-symbolic",
        }
    }
}

impl fmt::Display for PermissionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// User decision state for a permission
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    Allow,
    Block,
    Ask,
}

impl PermissionState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Block => "block",
            Self::Ask => "ask",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "allow" => Self::Allow,
            "block" => Self::Block,
            _ => Self::Ask,
        }
    }
}

/// Stored record of a site permission
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePermissionRecord {
    pub origin: String,
    pub permission_type: PermissionType,
    pub state: PermissionState,
    pub updated_at: i64,
}

/// SQLite repository for persistent site permissions
pub struct PermissionRepository<'a> {
    conn: &'a Connection,
}

impl<'a> PermissionRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Queries the stored decision for a given origin and permission type
    pub fn get_permission(
        &self,
        origin: &str,
        perm_type: PermissionType,
    ) -> SqlResult<Option<PermissionState>> {
        let mut stmt = self.conn.prepare(
            "SELECT state FROM site_permissions
             WHERE origin = ?1 AND permission_type = ?2;",
        )?;

        let mut rows = stmt.query(params![origin, perm_type.as_str()])?;
        if let Some(row) = rows.next()? {
            let state_str: String = row.get(0)?;
            Ok(Some(PermissionState::from_str(&state_str)))
        } else {
            Ok(None)
        }
    }

    /// Saves or updates a permission decision for an origin
    pub fn set_permission(
        &self,
        origin: &str,
        perm_type: PermissionType,
        state: PermissionState,
        persistence_allowed: bool,
    ) -> SqlResult<()> {
        if !persistence_allowed {
            info!("Bypassing permission persistence (private window active)");
            return Ok(());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO site_permissions (origin, permission_type, state, updated_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(origin, permission_type) DO UPDATE SET
                state = excluded.state,
                updated_at = excluded.updated_at;",
            params![origin, perm_type.as_str(), state.as_str(), now],
        )?;

        Ok(())
    }

    /// Deletes a specific permission record
    pub fn delete_permission(
        &self,
        origin: &str,
        perm_type: PermissionType,
        persistence_allowed: bool,
    ) -> SqlResult<()> {
        if !persistence_allowed {
            return Ok(());
        }

        self.conn.execute(
            "DELETE FROM site_permissions WHERE origin = ?1 AND permission_type = ?2;",
            params![origin, perm_type.as_str()],
        )?;

        Ok(())
    }

    /// Lists all stored site permissions for the Settings manager
    pub fn get_all_permissions(&self) -> SqlResult<Vec<SitePermissionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT origin, permission_type, state, updated_at
             FROM site_permissions ORDER BY origin ASC, permission_type ASC;",
        )?;

        let rows = stmt.query_map([], |row| {
            let origin: String = row.get(0)?;
            let perm_str: String = row.get(1)?;
            let state_str: String = row.get(2)?;
            let updated_at: i64 = row.get(3)?;

            Ok(SitePermissionRecord {
                origin,
                permission_type: PermissionType::from_str(&perm_str),
                state: PermissionState::from_str(&state_str),
                updated_at,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    /// Clears all stored permissions
    pub fn clear_all_permissions(&self, persistence_allowed: bool) -> SqlResult<()> {
        if !persistence_allowed {
            return Ok(());
        }

        self.conn.execute("DELETE FROM site_permissions;", [])?;
        Ok(())
    }
}

/// Identifies the permission category from a WebKit PermissionRequest
pub fn identify_permission_request(request: &PermissionRequest) -> (PermissionType, &'static str) {
    if request.is::<GeolocationPermissionRequest>() {
        (
            PermissionType::Geolocation,
            "wants to know your physical location",
        )
    } else if request.is::<NotificationPermissionRequest>() {
        (
            PermissionType::Notifications,
            "wants to display desktop notifications",
        )
    } else if let Some(media_req) = request.downcast_ref::<UserMediaPermissionRequest>() {
        let is_audio = webkit6::functions::user_media_permission_is_for_audio_device(media_req);
        let is_video = webkit6::functions::user_media_permission_is_for_video_device(media_req);

        if is_audio && is_video {
            (
                PermissionType::CameraAndMicrophone,
                "wants to use your camera and microphone",
            )
        } else if is_video {
            (PermissionType::Camera, "wants to use your camera")
        } else if is_audio {
            (PermissionType::Microphone, "wants to use your microphone")
        } else {
            (PermissionType::Other, "wants to access media devices")
        }
    } else if request.is::<ClipboardPermissionRequest>() {
        (PermissionType::Clipboard, "wants to access your clipboard")
    } else if request.is::<DeviceInfoPermissionRequest>() {
        (
            PermissionType::DeviceInfo,
            "wants access to audio/video device info",
        )
    } else {
        (PermissionType::Other, "wants access to hardware resources")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE site_permissions (
                origin TEXT NOT NULL,
                permission_type TEXT NOT NULL,
                state TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY (origin, permission_type)
            );",
            [],
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_permission_repository_crud() {
        let conn = setup_test_db();
        let repo = PermissionRepository::new(&conn);

        // Initially empty
        assert!(
            repo.get_permission("https://maps.google.com", PermissionType::Geolocation)
                .unwrap()
                .is_none()
        );

        // Set allow
        repo.set_permission(
            "https://maps.google.com",
            PermissionType::Geolocation,
            PermissionState::Allow,
            true,
        )
        .unwrap();

        assert_eq!(
            repo.get_permission("https://maps.google.com", PermissionType::Geolocation)
                .unwrap(),
            Some(PermissionState::Allow)
        );

        // Update to block
        repo.set_permission(
            "https://maps.google.com",
            PermissionType::Geolocation,
            PermissionState::Block,
            true,
        )
        .unwrap();

        assert_eq!(
            repo.get_permission("https://maps.google.com", PermissionType::Geolocation)
                .unwrap(),
            Some(PermissionState::Block)
        );

        // List
        let all = repo.get_all_permissions().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].origin, "https://maps.google.com");
        assert_eq!(all[0].permission_type, PermissionType::Geolocation);
        assert_eq!(all[0].state, PermissionState::Block);

        // Private mode persistence guard
        repo.set_permission(
            "https://private.com",
            PermissionType::Camera,
            PermissionState::Allow,
            false,
        )
        .unwrap();
        assert!(
            repo.get_permission("https://private.com", PermissionType::Camera)
                .unwrap()
                .is_none()
        );

        // Delete
        repo.delete_permission("https://maps.google.com", PermissionType::Geolocation, true)
            .unwrap();
        assert!(
            repo.get_permission("https://maps.google.com", PermissionType::Geolocation)
                .unwrap()
                .is_none()
        );
    }
}
