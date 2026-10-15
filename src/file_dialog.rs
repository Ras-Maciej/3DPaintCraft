// 3D Paintcraft - Visual File Explorer & Picker for Linux
// Pure safe Rust for egui

use std::fs;
use std::path::PathBuf;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileFilter {
    WavefrontObj,     // *.obj
    MeshFiles,        // *.obj, *.gltf, *.glb
    PaintcraftProject,// *.paintcraft
    AllFiles,
}

pub struct FileExplorerDialog {
    pub is_open: bool,
    pub title: String,
    pub current_dir: PathBuf,
    pub selected_file: Option<PathBuf>,
    pub filter: FileFilter,
    pub entries: Vec<DirEntryItem>,
    pub error_msg: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DirEntryItem {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub size_str: String,
}

impl Default for FileExplorerDialog {
    fn default() -> Self {
        let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        let mut dialog = Self {
            is_open: false,
            title: "Open 3D Model".to_string(),
            current_dir: dir,
            selected_file: None,
            filter: FileFilter::WavefrontObj,
            entries: Vec::new(),
            error_msg: None,
        };
        dialog.refresh_entries();
        dialog
    }
}

impl FileExplorerDialog {
    pub fn new_open_model() -> Self {
        let mut d = Self::default();
        d.title = "Open 3D Model (OBJ, glTF, GLB)".to_string();
        d.filter = FileFilter::MeshFiles;
        d.refresh_entries();
        d
    }

    pub fn new_open_project() -> Self {
        let mut d = Self::default();
        d.title = "Open Project (.paintcraft)".to_string();
        d.filter = FileFilter::PaintcraftProject;
        d.refresh_entries();
        d
    }

    pub fn open(&mut self) {
        self.is_open = true;
        self.selected_file = None;
        self.refresh_entries();
    }

    #[allow(dead_code)]
    pub fn close(&mut self) {
        self.is_open = false;
    }

    pub fn set_dir(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.current_dir = path;
            self.selected_file = None;
            self.refresh_entries();
        }
    }

    pub fn refresh_entries(&mut self) {
        self.entries.clear();
        self.error_msg = None;

        let read_res = fs::read_dir(&self.current_dir);
        let rd = match read_res {
            Ok(rd) => rd,
            Err(e) => {
                self.error_msg = Some(format!("Cannot open directory: {}", e));
                return;
            }
        };

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in rd.flatten() {
            let path = entry.path();
            let is_dir = path.is_dir();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();

            if name.starts_with('.') && name != ".gemini" {
                continue; // Skip hidden files except workspace root
            }

            if is_dir {
                dirs.push(DirEntryItem {
                    name,
                    path,
                    is_dir: true,
                    size_str: "<DIR>".to_string(),
                });
            } else {
                // Apply filter
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
                let matches = match self.filter {
                    FileFilter::WavefrontObj => ext == "obj",
                    FileFilter::MeshFiles => ext == "obj" || ext == "gltf" || ext == "glb",
                    FileFilter::PaintcraftProject => ext == "paintcraft",
                    FileFilter::AllFiles => true,
                };

                if matches {
                    let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                    let size_str = if size < 1024 {
                        format!("{} B", size)
                    } else if size < 1024 * 1024 {
                        format!("{:.1} KB", size as f64 / 1024.0)
                    } else {
                        format!("{:.1} MB", size as f64 / (1024.0 * 1024.0))
                    };

                    files.push(DirEntryItem {
                        name,
                        path,
                        is_dir: false,
                        size_str,
                    });
                }
            }
        }

        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        self.entries.extend(dirs);
        self.entries.extend(files);
    }

    /// Render interactive file explorer window in egui
    /// Returns Some(selected_path) when user clicks "Open" or double-clicks a file!
    pub fn ui(&mut self, ctx: &eframe::egui::Context) -> Option<PathBuf> {
        if !self.is_open {
            return None;
        }

        let mut confirmed_path = None;

        eframe::egui::Window::new(&self.title)
            .collapsible(false)
            .resizable(true)
            .default_size([720.0, 480.0])
            .show(ctx, |ui| {
                // Quick Location Bar
                ui.horizontal(|ui| {
                    ui.label("Quick Access:");
                    let home = std::env::var("USERPROFILE")
                        .or_else(|_| std::env::var("HOME"))
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

                    if ui.button("Home").clicked() {
                        self.set_dir(home.clone());
                    }
                    if ui.button("Workspace").clicked() {
                        if let Ok(cur) = std::env::current_dir() {
                            self.set_dir(cur);
                        }
                    }
                    let desktop = home.join("Desktop");
                    if desktop.exists() && ui.button("Desktop").clicked() {
                        self.set_dir(desktop);
                    }
                    let docs = home.join("Documents");
                    if docs.exists() && ui.button("Documents").clicked() {
                        self.set_dir(docs);
                    }
                    let downloads = home.join("Downloads");
                    if downloads.exists() && ui.button("Downloads").clicked() {
                        self.set_dir(downloads);
                    }
                });

                ui.separator();

                // Path & Parent Directory Navigation
                ui.horizontal(|ui| {
                    if ui.button("Up").on_hover_text("Parent directory").clicked() {
                        if let Some(parent) = self.current_dir.parent() {
                            let p = parent.to_path_buf();
                            self.set_dir(p);
                        }
                    }

                    let cur_str = self.current_dir.to_string_lossy().to_string();
                    ui.label(format!("Path: {}", cur_str));
                });

                ui.separator();

                if let Some(err) = &self.error_msg {
                    ui.colored_label(eframe::egui::Color32::from_rgb(255, 100, 100), err);
                }

                // File List Area
                let mut nav_dir = None;
                let mut select_file = None;
                let mut double_click_confirmed = None;

                eframe::egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        eframe::egui::Grid::new("file_explorer_grid")
                            .num_columns(3)
                            .spacing([20.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                ui.strong("Name");
                                ui.strong("Type");
                                ui.strong("Size");
                                ui.end_row();

                                for item in &self.entries {
                                    let badge = if item.is_dir {
                                        "[DIR]"
                                    } else if item.name.ends_with(".obj") {
                                        "[OBJ]"
                                    } else if item.name.ends_with(".paintcraft") {
                                        "[PROJ]"
                                    } else {
                                        "[FILE]"
                                    };

                                    let is_selected = self.selected_file.as_ref() == Some(&item.path);
                                    let label_text = format!("{} {}", badge, item.name);

                                    let response = ui.selectable_label(is_selected, label_text);

                                    if response.double_clicked() {
                                        if item.is_dir {
                                            nav_dir = Some(item.path.clone());
                                        } else {
                                            double_click_confirmed = Some(item.path.clone());
                                        }
                                    } else if response.clicked() {
                                        if item.is_dir {
                                            nav_dir = Some(item.path.clone());
                                        } else {
                                            select_file = Some(item.path.clone());
                                        }
                                    }

                                    ui.label(if item.is_dir { "Folder" } else { "File" });
                                    ui.label(&item.size_str);
                                    ui.end_row();
                                }
                            });
                    });

                if let Some(p) = nav_dir {
                    self.set_dir(p);
                }
                if let Some(p) = select_file {
                    self.selected_file = Some(p);
                }
                if let Some(p) = double_click_confirmed {
                    confirmed_path = Some(p);
                    self.is_open = false;
                }

                ui.separator();

                // Bottom Action Bar
                ui.horizontal(|ui| {
                    if let Some(sel) = &self.selected_file {
                        let name = sel.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        ui.label(format!("Selected: {}", name));
                    } else {
                        ui.label("No file selected");
                    }

                    ui.with_layout(eframe::egui::Layout::right_to_left(eframe::egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            self.is_open = false;
                        }

                        let can_confirm = self.selected_file.is_some();
                        if ui.add_enabled(can_confirm, eframe::egui::Button::new("Open")).clicked() {
                            if let Some(sel) = self.selected_file.clone() {
                                confirmed_path = Some(sel);
                                self.is_open = false;
                            }
                        }
                    });
                });
            });

        confirmed_path
    }
}
