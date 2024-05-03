#![windows_subsystem = "windows"]

use eframe::egui::{vec2, Color32, Stroke};
use git2::Repository;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(target_os = "windows")]
const FALLBACK_FONT: &str = "C:\\Windows\\Fonts\\msyh.ttc";

#[cfg(target_os = "macos")]
const FALLBACK_FONT: &str = "/System/Library/Fonts/PingFang.ttc";

struct App {
    projects: Vec<Project>,
    new_project: Project,
    selected_projects: Vec<bool>,
    progress: f32,
    log_buffer: String,
    config_path: PathBuf,
    font_size: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Project {
    path: String,
    name: String,
    notes: String,
}

impl Default for App {
    fn default() -> Self {
        let config_dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
        let config_path = config_dir.join("github_project_manager.json");

        let projects = match std::fs::read_to_string(&config_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => Vec::new(),
        };

        let selected_projects_len = projects.len();

        Self {
            projects,
            new_project: Project {
                path: "".to_owned(),
                name: "".to_owned(),
                notes: "".to_owned(),
            },
            selected_projects: vec![false; selected_projects_len],
            progress: 0.0,
            log_buffer: String::new(),
            config_path,
            font_size: 16.0,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        let window_size = frame.info().window_info.size;
        self.font_size = (window_size.x / 50.0).clamp(12.0, 24.0);

        let mut style = (*ctx.style()).clone();
        style.text_styles = [
            (egui::TextStyle::Heading, egui::FontId::new(self.font_size * 1.2, egui::FontFamily::Proportional)),
            (egui::TextStyle::Body, egui::FontId::new(self.font_size, egui::FontFamily::Proportional)),
            (egui::TextStyle::Monospace, egui::FontId::new(self.font_size, egui::FontFamily::Monospace)),
            (egui::TextStyle::Button, egui::FontId::new(self.font_size, egui::FontFamily::Proportional)),
            (egui::TextStyle::Small, egui::FontId::new(self.font_size * 0.8, egui::FontFamily::Proportional)),
        ]
            .into();
        ctx.set_style(style);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("这是一个简单的GitHub项目管理工具,可以用来更新多个项目的代码");

            ui.horizontal(|ui| {
                ui.label("项目路径:");
                ui.text_edit_singleline(&mut self.new_project.path);
            });

            ui.horizontal(|ui| {
                ui.label("项目名称:");
                ui.text_edit_singleline(&mut self.new_project.name);
            });

            ui.horizontal(|ui| {
                ui.label("项目备注:");
                ui.text_edit_singleline(&mut self.new_project.notes);
            });

            if ui.add(egui::Button::new("添加项目").stroke(Stroke::new(2.0, Color32::GRAY))).clicked() {
                if !self.new_project.path.is_empty() && !self.new_project.name.is_empty() {
                    if let Ok(repo) = Repository::open(&self.new_project.path) {
                        if repo.find_remote("origin").is_ok() {
                            self.projects.push(self.new_project.clone());
                            self.selected_projects.push(false);
                            self.new_project.path.clear();
                            self.new_project.name.clear();
                            self.new_project.notes.clear();
                            self.save_config();
                        } else {
                            self.log_error(format!("项目 {} 不是一个有效的Git仓库或没有origin远程仓库", self.new_project.name));
                        }
                    } else {
                        self.log_error(format!("项目路径 {} 不存在或不是一个有效的Git仓库", self.new_project.path));
                    }
                } else {
                    self.log_error("项目路径和名称不能为空".to_string());
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui.add(egui::Button::new("更新选中项目").stroke(Stroke::new(2.0, Color32::GRAY))).clicked() {
                    self.update_selected_projects();
                }

                if ui.add(egui::Button::new("删除选中项目").stroke(Stroke::new(2.0, Color32::GRAY))).clicked() {
                    self.delete_selected_projects();
                }
            });

            ui.separator();

            egui::ScrollArea::new([false, true]).id_source("project_list").show(ui, |ui| {
                for (i, project) in self.projects.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.selected_projects[i], "");
                        ui.label(&project.name);
                    });
                    ui.label(&project.path);
                    ui.label(&project.notes);
                    ui.separator();
                }
            });

            ui.separator();

            ui.label(format!("进度: {}%", (self.progress * 100.0) as u32));
            ui.add(egui::ProgressBar::new(self.progress).show_percentage());

            ui.separator();

            egui::ScrollArea::new([false, true]).id_source("log_area")
                .max_height(200.0)
                .show(ui, |ui| {
                    ui.text_edit_multiline(&mut self.log_buffer);
                });
        });

        frame.set_window_size(ctx.used_size());
    }

    fn save(&mut self, _storage: &mut dyn eframe::Storage) {
        self.save_config();
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_config();
    }
}

impl App {
    fn update_selected_projects(&mut self) {
        let selected_projects: Vec<_> = self.selected_projects.iter().enumerate()
            .filter(|(_, &selected)| selected)
            .map(|(index, _)| index)
            .collect();

        let total_projects = selected_projects.len() as f32;
        let mut completed_projects = 0.0;
        let mut log_messages = Vec::new();

        for &index in &selected_projects {
            if let Some(project) = self.projects.get_mut(index) {
                if let Ok(repo) = Repository::open(&project.path) {
                    if let Ok(mut remote) = repo.find_remote("origin") {
                        if let Err(e) = remote.fetch(&["master"], None, None) {
                            log_messages.push(format!("[ERROR] 无法获取远程更新: {}", e));
                        } else {
                            if let Ok(fetch_head) = repo.find_reference("FETCH_HEAD") {
                                let fetch_commit = repo.reference_to_annotated_commit(&fetch_head).unwrap();
                                let analysis = repo.merge_analysis(&[&fetch_commit]).unwrap();

                                if analysis.0.is_up_to_date() {
                                    log_messages.push(format!("[INFO] 项目 {} 已经是最新版本", project.name));
                                } else if analysis.0.is_fast_forward() {
                                    let refname = "refs/heads/master";
                                    let mut reference = repo.find_reference(refname).unwrap();
                                    reference.set_target(fetch_commit.id(), "Fast-Forward").unwrap();
                                    repo.set_head(refname).unwrap();
                                    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force())).unwrap();
                                    log_messages.push(format!("[INFO] 项目 {} 更新成功", project.name));
                                } else {
                                    log_messages.push(format!("[ERROR] 项目 {} 存在冲突,需要手动解决", project.name));
                                }
                            } else {
                                log_messages.push(format!("[ERROR] 项目 {} 的 FETCH_HEAD 文件损坏或不存在", project.name));
                            }
                        }
                    } else {
                        log_messages.push(format!("[ERROR] 无法找到远程仓库'origin': {}", project.name));
                    }
                } else {
                    log_messages.push(format!("[ERROR] 无法打开仓库: {}", project.path));
                }
            }
            completed_projects += 1.0;
            self.progress = completed_projects / total_projects;
        }

        for message in log_messages {
            self.log_buffer.push_str(&format!("{}\n", message));
        }
        self.limit_log_buffer();

        let mut selected_projects = std::mem::take(&mut self.selected_projects);
        for selected in &mut selected_projects {
            *selected = false;
        }
        self.selected_projects = selected_projects;
    }

    fn delete_selected_projects(&mut self) {
        let mut indices_to_remove = Vec::new();
        for (i, &selected) in self.selected_projects.iter().enumerate().rev() {
            if selected {
                indices_to_remove.push(i);
            }
        }

        for index in indices_to_remove {
            self.projects.remove(index);
            self.selected_projects.remove(index);
        }

        self.save_config();
    }

    fn log_error(&mut self, message: String) {
        self.log_buffer.push_str(&format!("[ERROR] {}\n", message));
        self.limit_log_buffer();
    }

    fn limit_log_buffer(&mut self) {
        let max_lines = 1000;
        let lines: Vec<&str> = self.log_buffer.lines().collect();
        if lines.len() > max_lines {
            let skip_lines = lines.len() - max_lines;
            self.log_buffer = lines[skip_lines..].join("\n");
        }
    }

    fn save_config(&self) {
        if let Ok(config) = serde_json::to_string_pretty(&self.projects) {
            if let Err(e) = std::fs::write(&self.config_path, config) {
                eprintln!("无法保存配置文件: {}", e);
            }
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        resizable: true,
        initial_window_size: Some(vec2(800.0, 600.0)),
        ..Default::default()
    };

    let mut fonts = egui::FontDefinitions::default();
    if let Some(font_data) = load_fallback_font() {
        fonts.font_data.insert("fallback".to_owned(), font_data);
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "fallback".to_owned());
    }

    eframe::run_native(
        "GitHub项目管理工具-By：刘一手",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_fonts(fonts);
            cc.egui_ctx.set_pixels_per_point(1.25);
            Box::new(App::default())
        }),
    );
}

fn load_fallback_font() -> Option<egui::FontData> {
    if let Ok(font_data) = std::fs::read(FALLBACK_FONT) {
        Some(egui::FontData::from_owned(font_data))
    } else {
        eprintln!("无法加载字体");
        None
    }
}



