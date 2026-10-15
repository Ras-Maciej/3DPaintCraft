// 3D Paintcraft - GitHub Release Update Notifier & Browser Launcher
// Pure safe Rust with egui integration

use eframe::egui;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const DEFAULT_REPO: &str = "ArtCraft-Suite/3d-paintcraft";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

impl GitHubReleaseAsset {
    pub fn size_formatted(&self) -> String {
        if self.size >= 1024 * 1024 {
            format!("{:.1} MB", self.size as f64 / (1024.0 * 1024.0))
        } else if self.size >= 1024 {
            format!("{:.0} KB", self.size as f64 / 1024.0)
        } else {
            format!("{} B", self.size)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub html_url: String,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GitHubReleaseAsset>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        checked_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        release: GitHubRelease,
    },
    Error(String),
}

pub struct UpdateNotifier {
    pub repo: String,
    pub status: Arc<Mutex<UpdateStatus>>,
    pub show_dialog: bool,
    pub auto_checked_startup: bool,
}

impl Default for UpdateNotifier {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdateNotifier {
    pub fn new() -> Self {
        Self {
            repo: DEFAULT_REPO.to_string(),
            status: Arc::new(Mutex::new(UpdateStatus::Idle)),
            show_dialog: false,
            auto_checked_startup: false,
        }
    }

    /// Check if a newer version is currently available
    pub fn has_update_available(&self) -> bool {
        if let Ok(guard) = self.status.lock() {
            matches!(*guard, UpdateStatus::UpdateAvailable { .. })
        } else {
            false
        }
    }

    /// Query GitHub Releases API on a background thread
    pub fn check_for_updates(&mut self, is_manual: bool) {
        let repo = self.repo.trim().to_string();
        let status = Arc::clone(&self.status);
        if let Ok(mut guard) = status.lock() {
            *guard = UpdateStatus::Checking;
        }

        if is_manual {
            self.show_dialog = true;
        }

        thread::spawn(move || {
            let res = fetch_latest_release(&repo);
            if let Ok(mut guard) = status.lock() {
                match res {
                    Ok(release) => {
                        if is_newer_version(&release.tag_name, CURRENT_VERSION) {
                            *guard = UpdateStatus::UpdateAvailable {
                                current_version: CURRENT_VERSION.to_string(),
                                latest_version: release.tag_name.clone(),
                                release,
                            };
                        } else {
                            *guard = UpdateStatus::UpToDate {
                                checked_version: release.tag_name,
                            };
                        }
                    }
                    Err(err) => {
                        *guard = UpdateStatus::Error(err);
                    }
                }
            }
        });
    }

    /// Render update modal dialog in egui
    pub fn render_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_dialog {
            return;
        }

        let mut is_open = self.show_dialog;
        let mut trigger_recheck = false;

        egui::Window::new("Check for Updates - 3D Paintcraft")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add_space(4.0);

                let current_status = if let Ok(guard) = self.status.lock() {
                    guard.clone()
                } else {
                    UpdateStatus::Idle
                };

                match current_status {
                    UpdateStatus::Idle => {
                        ui.horizontal(|ui| {
                            ui.label("Current Version:");
                            ui.strong(format!("v{}", CURRENT_VERSION));
                        });
                        ui.add_space(8.0);
                        ui.label("Click below to check GitHub for newer releases and updates.");
                        ui.add_space(8.0);
                        if ui.button("Check Now").clicked() {
                            trigger_recheck = true;
                        }
                    }
                    UpdateStatus::Checking => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(format!("Checking https://github.com/{} for releases...", self.repo));
                        });
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new("Querying GitHub Releases API...").small().color(egui::Color32::GRAY));
                    }
                    UpdateStatus::UpToDate { checked_version } => {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("3D Paintcraft is Up to Date").color(egui::Color32::from_rgb(46, 204, 113)).heading());
                        });
                        ui.add_space(8.0);
                        ui.label(format!("You are running the latest version (v{}) matching GitHub release {}.", CURRENT_VERSION, checked_version));
                        ui.add_space(8.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Check Again").clicked() {
                                trigger_recheck = true;
                            }
                            if ui.button("Close").clicked() {
                                self.show_dialog = false;
                            }
                        });
                    }
                    UpdateStatus::UpdateAvailable {
                        current_version,
                        latest_version,
                        release,
                    } => {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("A New Update is Available!").color(egui::Color32::from_rgb(243, 94, 24)).heading());
                        });
                        ui.add_space(6.0);

                        egui::Grid::new("update_version_grid").num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                            ui.label("Installed Version:");
                            ui.label(egui::RichText::new(format!("v{}", current_version)).strong());
                            ui.end_row();

                            ui.label("Latest Available:");
                            ui.label(egui::RichText::new(&latest_version).color(egui::Color32::from_rgb(46, 204, 113)).strong());
                            ui.end_row();

                            if let Some(pub_date) = &release.published_at {
                                ui.label("Release Date:");
                                ui.label(pub_date.chars().take(10).collect::<String>());
                                ui.end_row();
                            }
                        });

                        ui.add_space(8.0);
                        if let Some(title) = &release.name {
                            ui.strong(title);
                        }

                        if let Some(body) = &release.body {
                            ui.group(|ui| {
                                ui.label(egui::RichText::new("Release Notes & Changelog:").small().strong());
                                egui::ScrollArea::vertical()
                                    .max_height(140.0)
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.label(body);
                                    });
                            });
                        }

                        if !release.assets.is_empty() {
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new("Available Downloads:").small().strong());
                            for asset in &release.assets {
                                ui.horizontal(|ui| {
                                    ui.label(format!("- {} ({})", asset.name, asset.size_formatted()));
                                    if ui.button("Download").clicked() {
                                        ctx.open_url(egui::OpenUrl::new_tab(&asset.browser_download_url));
                                    }
                                });
                            }
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            let open_btn = egui::Button::new(
                                egui::RichText::new("Open GitHub Release Page").strong().color(egui::Color32::WHITE)
                            ).fill(egui::Color32::from_rgb(243, 94, 24));

                            if ui.add(open_btn).clicked() {
                                ctx.open_url(egui::OpenUrl::new_tab(&release.html_url));
                            }

                            if ui.button("Remind Me Later").clicked() {
                                self.show_dialog = false;
                            }
                        });
                    }
                    UpdateStatus::Error(ref err_msg) => {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Unable to Check for Updates").color(egui::Color32::from_rgb(230, 70, 70)).heading());
                        });
                        ui.add_space(6.0);
                        ui.label(err_msg);

                        ui.add_space(10.0);
                        ui.group(|ui| {
                            ui.label(egui::RichText::new("GitHub Repository Settings:").small().strong());
                            ui.horizontal(|ui| {
                                ui.label("Repo:");
                                ui.text_edit_singleline(&mut self.repo);
                            });
                            ui.label(egui::RichText::new("Format: username/repository (e.g. YourName/3d-paintcraft)").small().color(egui::Color32::GRAY));
                        });

                        ui.add_space(8.0);
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button("Retry Check").clicked() {
                                trigger_recheck = true;
                            }
                            if ui.button("Close").clicked() {
                                self.show_dialog = false;
                            }
                        });
                    }
                }
            });

        self.show_dialog = is_open;

        if trigger_recheck {
            self.check_for_updates(true);
        }
    }
}

/// Parse semver strings (e.g., "v0.2.0" or "1.4.12")
pub fn parse_semver(s: &str) -> (u32, u32, u32) {
    let clean = s.trim().trim_start_matches('v').trim_start_matches('V');
    let mut parts = clean.split('.');
    let major = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let patch = parts
        .next()
        .and_then(|x| x.split('-').next().unwrap_or(x).parse().ok())
        .unwrap_or(0);
    (major, minor, patch)
}

/// Determine if `latest` is strictly newer than `current`
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    parse_semver(latest) > parse_semver(current)
}

/// Fetch latest release from GitHub API using platform native tools
fn fetch_latest_release(repo: &str) -> Result<GitHubRelease, String> {
    let repo_clean = repo.trim();
    if repo_clean.is_empty() || !repo_clean.contains('/') {
        return Err("Please specify a valid GitHub repository in the format 'username/repo'.".to_string());
    }

    let url = format!("https://api.github.com/repos/{}/releases/latest", repo_clean);

    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg("-H")
        .arg("User-Agent: 3D-Paintcraft-App")
        .arg("-H")
        .arg("Accept: application/vnd.github.v3+json")
        .arg("--max-time")
        .arg("8")
        .arg(&url)
        .output();

    let body = match output {
        Ok(out) if out.status.success() => {
            String::from_utf8(out.stdout).map_err(|e| format!("Invalid UTF-8 from GitHub API: {}", e))?
        }
        Ok(out) => {
            #[cfg(target_os = "windows")]
            {
                let ps_cmd = format!("(Invoke-WebRequest -Uri '{}' -UseBasicParsing -Headers @{{'User-Agent'='3D-Paintcraft-App'}}).Content", url);
                let ps_out = std::process::Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(&ps_cmd)
                    .output()
                    .map_err(|e| format!("PowerShell fallback failed: {}", e))?;
                if ps_out.status.success() {
                    String::from_utf8(ps_out.stdout).map_err(|e| format!("Invalid UTF-8 from PowerShell: {}", e))?
                } else {
                    return Err(format!("curl returned exit code: {}", out.status));
                }
            }
            #[cfg(not(target_os = "windows"))]
            return Err(format!("curl request failed with exit code: {}", out.status));
        }
        Err(e) => {
            #[cfg(target_os = "windows")]
            {
                let ps_cmd = format!("(Invoke-WebRequest -Uri '{}' -UseBasicParsing -Headers @{{'User-Agent'='3D-Paintcraft-App'}}).Content", url);
                let ps_out = std::process::Command::new("powershell")
                    .arg("-NoProfile")
                    .arg("-Command")
                    .arg(&ps_cmd)
                    .output()
                    .map_err(|err| format!("Failed to run curl ({}) and PowerShell ({})", e, err))?;
                if ps_out.status.success() {
                    String::from_utf8(ps_out.stdout).map_err(|err| format!("Invalid UTF-8 from PowerShell: {}", err))?
                } else {
                    return Err(format!("Failed to connect to GitHub: {}", e));
                }
            }
            #[cfg(not(target_os = "windows"))]
            return Err(format!("Failed to execute curl: {}", e));
        }
    };

    if body.contains("\"message\": \"Not Found\"") {
        return Err(format!(
            "No releases found for GitHub repository '{}'. Make sure the repository exists, is public, and has at least one published release.",
            repo_clean
        ));
    }

    serde_json::from_str::<GitHubRelease>(&body)
        .map_err(|e| format!("Failed to parse GitHub release data: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semver_parsing() {
        assert_eq!(parse_semver("v0.1.0"), (0, 1, 0));
        assert_eq!(parse_semver("0.2.5"), (0, 2, 5));
        assert_eq!(parse_semver("v1.0.0-rc1"), (1, 0, 0));
        assert_eq!(parse_semver("V2.10.3"), (2, 10, 3));
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.2.0", "0.1.0"));
        assert!(is_newer_version("v1.0.0", "0.9.9"));
        assert!(is_newer_version("0.1.1", "0.1.0"));
        assert!(!is_newer_version("v0.1.0", "0.1.0"));
        assert!(!is_newer_version("v0.0.9", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.2.0"));
    }

    #[test]
    fn test_update_notifier_default() {
        let notifier = UpdateNotifier::new();
        assert_eq!(notifier.repo, DEFAULT_REPO);
        assert!(!notifier.has_update_available());
        assert!(!notifier.show_dialog);
    }
}
