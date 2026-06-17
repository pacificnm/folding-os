use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppliancePaths {
    pub config_dir: PathBuf,
    pub defaults_dir: PathBuf,
    pub effective_dir: PathBuf,
    pub active_installation_role: PathBuf,
    pub foldops_embedded_manifest: PathBuf,
    pub foldops_assigned_manifest: PathBuf,
    pub foldops_apps_root: PathBuf,
    pub foldops_provisioned_marker: PathBuf,
    pub tools_bootstrap_manifest: PathBuf,
    pub tools_assigned_version: PathBuf,
    pub tools_active_state: PathBuf,
    pub tools_binary: PathBuf,
    pub fah_apps_root: PathBuf,
    pub fah_embedded_manifest: PathBuf,
    pub fah_log: PathBuf,
    pub staged_update_meta: PathBuf,
    pub pending_update_report: PathBuf,
    pub reboot_required: PathBuf,
    pub supervisor_url: PathBuf,
    pub enrollment_token: PathBuf,
}

impl Default for AppliancePaths {
    fn default() -> Self {
        Self {
            config_dir: PathBuf::from("/data/config"),
            defaults_dir: PathBuf::from("/etc/foldingos/defaults"),
            effective_dir: PathBuf::from("/run/foldingos/effective"),
            active_installation_role: PathBuf::from("/data/config/installation-role"),
            foldops_embedded_manifest: PathBuf::from("/usr/share/foldingos/manifests/foldops.toml"),
            foldops_assigned_manifest: PathBuf::from("/data/config/foldops/assigned-manifest.toml"),
            foldops_apps_root: PathBuf::from("/data/apps/foldops"),
            foldops_provisioned_marker: PathBuf::from("/data/state/foldops/provisioned.json"),
            tools_bootstrap_manifest: PathBuf::from("/usr/share/foldingos/manifests/tools.json"),
            tools_assigned_version: PathBuf::from("/data/config/tools/assigned-version.json"),
            tools_active_state: PathBuf::from("/data/state/tools/active.json"),
            tools_binary: PathBuf::from("/usr/bin/foldingosctl"),
            fah_apps_root: PathBuf::from("/data/apps/fah"),
            fah_embedded_manifest: PathBuf::from("/usr/share/foldingos/manifests/fah.toml"),
            fah_log: PathBuf::from("/data/fah/log.txt"),
            staged_update_meta: PathBuf::from("/data/state/provision/staged-update.json"),
            pending_update_report: PathBuf::from("/data/state/provision/pending-update-report.json"),
            reboot_required: PathBuf::from("/run/reboot-required"),
            supervisor_url: PathBuf::from("/data/config/provision/supervisor.url"),
            enrollment_token: PathBuf::from("/data/config/provision/enrollment-token"),
        }
    }
}

impl AppliancePaths {
    pub fn node_id_path(&self) -> PathBuf {
        self.config_dir.join("node-id")
    }

    pub fn system_config_path(&self) -> PathBuf {
        self.config_dir.join("system.toml")
    }

    pub fn system_defaults_path(&self) -> PathBuf {
        self.defaults_dir.join("system.toml")
    }

    pub fn system_overrides_path(&self) -> PathBuf {
        self.config_dir.join("overrides/system.toml")
    }

    pub fn effective_system_path(&self) -> PathBuf {
        self.effective_dir.join("system.toml")
    }

    pub fn foldops_current_link(&self) -> PathBuf {
        self.foldops_apps_root.join("current")
    }

    pub fn fah_current_link(&self) -> PathBuf {
        self.fah_apps_root.join("current")
    }

    pub fn path_exists(&self, path: &Path) -> bool {
        path.exists()
    }
}
