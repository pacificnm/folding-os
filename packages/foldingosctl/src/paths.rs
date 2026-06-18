use std::path::PathBuf;

pub const FAH_VERIFIED_MARKER: &str = ".foldingos-verified";

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
    pub fah_acquire_state: PathBuf,
    pub fah_runtime_config: PathBuf,
    pub fah_log: PathBuf,
    pub staged_update_meta: PathBuf,
    pub pending_update_report: PathBuf,
    pub reboot_required: PathBuf,
    pub supervisor_url: PathBuf,
    pub enrollment_token: PathBuf,
    pub provision_enrollments_dir: PathBuf,
    pub provision_enrollments_index: PathBuf,
    pub boot_allowlist: PathBuf,
    pub boot_install_disk_allowlist: PathBuf,
    pub automation_policy: PathBuf,
    pub agent_automation_policy: PathBuf,
    pub registry_index: PathBuf,
    pub registry_entries_dir: PathBuf,
    pub registry_images_dir: PathBuf,
    pub upstream_releases_url: PathBuf,
    pub embedded_build_revision: PathBuf,
    pub foldops_registry_index: PathBuf,
    pub foldops_registry_releases_dir: PathBuf,
    pub tools_registry_index: PathBuf,
    pub tools_registry_releases_dir: PathBuf,
    pub provisioned_ssh_keys: PathBuf,
    pub active_ssh_keys: PathBuf,
    pub ssh_host_key: PathBuf,
    pub provisioned_installation_role: PathBuf,
    pub agent_enrollment_state: PathBuf,
    pub provision_listen_url: PathBuf,
    pub provision_sessions_dir: PathBuf,
    pub staged_update_image: PathBuf,
    pub staged_update_partial: PathBuf,
    pub staged_update_lock: PathBuf,
    pub provision_boot_tftp_root: PathBuf,
    pub provision_boot_interface: PathBuf,
    pub provision_boot_dnsmasq_config: PathBuf,
    pub provision_boot_isolated_network: PathBuf,
    pub provision_boot_assets_dir: PathBuf,
    pub update_grub_env: PathBuf,
    pub update_boot_assets_dir: PathBuf,
    pub shared_update_vmlinuz: PathBuf,
    pub shared_update_initramfs: PathBuf,
    pub foldops_ingest_token: PathBuf,
    pub foldops_tls_dir: PathBuf,
    pub foldops_db: PathBuf,
    pub foldops_backups_dir: PathBuf,
    pub foldops_config_dir: PathBuf,
    pub data_root: PathBuf,
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
            fah_acquire_state: PathBuf::from("/data/state/fah-acquire.state"),
            fah_runtime_config: PathBuf::from("/run/foldingos/fah/config.xml"),
            fah_log: PathBuf::from("/data/fah/log.txt"),
            staged_update_meta: PathBuf::from("/data/state/provision/staged-update.json"),
            pending_update_report: PathBuf::from("/data/state/provision/pending-update-report.json"),
            reboot_required: PathBuf::from("/run/reboot-required"),
            supervisor_url: PathBuf::from("/data/config/provision/supervisor.url"),
            enrollment_token: PathBuf::from("/data/config/provision/enrollment-token"),
            provision_enrollments_dir: PathBuf::from("/data/provision/enrollments"),
            provision_enrollments_index: PathBuf::from("/data/provision/enrollments/index.json"),
            boot_allowlist: PathBuf::from("/data/config/provision/boot-allowlist"),
            boot_install_disk_allowlist: PathBuf::from("/data/config/provision/boot-install-disk-allowlist"),
            automation_policy: PathBuf::from("/usr/share/foldingos/foldops-supervisor-automation.toml"),
            agent_automation_policy: PathBuf::from("/usr/share/foldingos/foldops-agent-automation.toml"),
            registry_index: PathBuf::from("/data/registry/index.json"),
            registry_entries_dir: PathBuf::from("/data/registry/entries"),
            registry_images_dir: PathBuf::from("/data/registry/images"),
            upstream_releases_url: PathBuf::from("/data/config/provision/upstream-releases.url"),
            embedded_build_revision: PathBuf::from("/usr/share/foldingos/build-revision"),
            foldops_registry_index: PathBuf::from("/data/registry/foldops/index.json"),
            foldops_registry_releases_dir: PathBuf::from("/data/registry/foldops/releases"),
            tools_registry_index: PathBuf::from("/data/registry/tools/index.json"),
            tools_registry_releases_dir: PathBuf::from("/data/registry/tools/releases"),
            provisioned_ssh_keys: PathBuf::from("/boot/efi/foldingos/provision/authorized_keys"),
            active_ssh_keys: PathBuf::from("/data/config/ssh/authorized_keys"),
            ssh_host_key: PathBuf::from("/data/config/ssh/host-keys/ssh_host_ed25519_key"),
            provisioned_installation_role: PathBuf::from(
                "/boot/efi/foldingos/provision/installation-role",
            ),
            agent_enrollment_state: PathBuf::from("/data/state/provision/enrolled"),
            provision_listen_url: PathBuf::from("/data/config/provision/listen.url"),
            provision_sessions_dir: PathBuf::from("/data/provision/sessions"),
            staged_update_image: PathBuf::from("/data/state/provision/staged-update.img"),
            staged_update_partial: PathBuf::from("/data/state/provision/staged-update.partial"),
            staged_update_lock: PathBuf::from("/data/state/provision/staged-update.lock"),
            provision_boot_tftp_root: PathBuf::from("/data/provision/boot/tftp"),
            provision_boot_interface: PathBuf::from("/data/config/provision/boot.interface"),
            provision_boot_dnsmasq_config: PathBuf::from("/data/config/provision/dnsmasq.conf"),
            provision_boot_isolated_network: PathBuf::from(
                "/data/config/provision/boot-isolated-network",
            ),
            provision_boot_assets_dir: PathBuf::from("/usr/share/foldingos/boot"),
            update_grub_env: PathBuf::from("/boot/efi/EFI/BOOT/grubenv"),
            update_boot_assets_dir: PathBuf::from("/boot/efi/foldingos/update"),
            shared_update_vmlinuz: PathBuf::from("/usr/share/foldingos/boot/vmlinuz"),
            shared_update_initramfs: PathBuf::from(
                "/usr/share/foldingos/boot/install-initramfs.cpio.gz",
            ),
            foldops_ingest_token: PathBuf::from("/data/config/foldops/ingest-token"),
            foldops_tls_dir: PathBuf::from("/data/foldops/tls"),
            foldops_db: PathBuf::from("/data/foldops/foldops.db"),
            foldops_backups_dir: PathBuf::from("/data/foldops/backups"),
            foldops_config_dir: PathBuf::from("/data/config/foldops"),
            data_root: PathBuf::from("/data"),
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

    pub fn fah_downloads_dir(&self) -> PathBuf {
        self.fah_apps_root.join(".downloads")
    }

    pub fn fah_runtime_dir(&self) -> PathBuf {
        self.fah_runtime_config
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("/run/foldingos/fah"))
    }

    pub fn fah_version_dir(&self, version: &str) -> PathBuf {
        self.fah_apps_root.join(version)
    }

    pub fn fah_staging_dir(&self, version: &str) -> PathBuf {
        self.fah_apps_root.join(format!("{version}.staging"))
    }

    pub fn fah_staged_deb(&self, version: &str) -> PathBuf {
        self.fah_downloads_dir().join(format!("{version}.deb"))
    }

    pub fn fah_partial_deb(&self, version: &str) -> PathBuf {
        self.fah_downloads_dir().join(format!("{version}.partial"))
    }

    pub fn fah_verified_marker(&self, version: &str) -> PathBuf {
        self.fah_version_dir(version).join(FAH_VERIFIED_MARKER)
    }

    pub fn enrollment_record_path(&self, node_id: &str) -> PathBuf {
        self.provision_enrollments_dir.join(format!("{node_id}.json"))
    }

    pub fn registry_entry_path(&self, version: &str) -> PathBuf {
        self.registry_entries_dir.join(format!("{version}.json"))
    }

    pub fn foldops_registry_entry_path(&self, release: &str) -> PathBuf {
        self.foldops_registry_releases_dir.join(format!("{release}.json"))
    }

    pub fn tools_registry_entry_path(&self, version: &str) -> PathBuf {
        self.tools_registry_releases_dir.join(format!("{version}.json"))
    }

    pub fn install_session_path(&self, session_id: &str) -> PathBuf {
        self.provision_sessions_dir.join(format!("{session_id}.json"))
    }

    pub fn update_session_path(&self, session_id: &str) -> PathBuf {
        self.provision_sessions_dir
            .join(format!("update-{session_id}.json"))
    }

    pub fn ssh_host_key_pub(&self) -> PathBuf {
        PathBuf::from(format!("{}.pub", self.ssh_host_key.display()))
    }

    pub fn foldops_supervisor_ca_pem(&self) -> PathBuf {
        self.config_dir.join("foldops/supervisor-ca.pem")
    }

    pub fn domain_active_path(&self, domain: &str) -> PathBuf {
        self.config_dir.join(format!("{domain}.toml"))
    }

    pub fn domain_defaults_path(&self, domain: &str) -> PathBuf {
        self.defaults_dir.join(format!("{domain}.toml"))
    }

    pub fn domain_overrides_path(&self, domain: &str) -> PathBuf {
        self.config_dir.join(format!("overrides/{domain}.toml"))
    }

    pub fn domain_effective_path(&self, domain: &str) -> PathBuf {
        self.effective_dir.join(format!("{domain}.toml"))
    }

    pub fn domain_last_good_path(&self, domain: &str) -> PathBuf {
        self.config_dir.join(format!("last-good/{domain}.toml"))
    }

    pub fn domain_lock_path(&self, domain: &str) -> PathBuf {
        PathBuf::from(format!("/run/lock/foldingos-config-{domain}.lock"))
    }

    pub fn secrets_dir(&self) -> PathBuf {
        self.config_dir.join("secrets")
    }
}
