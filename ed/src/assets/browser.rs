use std::path::{PathBuf, absolute};

use arcana::{
    assets::import::{ImporterDesc, ImporterId},
    error::{Error, fail},
    model::Value,
    project::Project,
};
use egui::Ui;
use egui_file::FileDialog;
use hashbrown::HashMap;
use smallvec::SmallVec;

use crate::{assets::AssetStore, model::ValueProbe};

enum ImportState {
    PickAsset(PickAsset),
    Configure(Configure),
}

pub struct AssetBrowser {
    store: AssetStore,
    import_state: Option<ImportState>,
}

impl AssetBrowser {
    pub fn show(&mut self, ui: &mut Ui, project: &Project) -> Result<(), Error> {
        match &mut self.import_state {
            None => {}
            Some(ImportState::PickAsset(pick_asset)) => {
                pick_asset.show(ui.ctx(), project);
                match pick_asset.take_selected() {
                    None => {}
                    Some(selected_asset) => {}
                }
            }
            Some(ImportState::Configure(configure)) => {}
        }

        // if !self.config.is_open() {
        //     if let Some(source) = self.pick_asset.take_selected() {
        //         let res = absolute(&source).ok();

        //         match res {
        //             None => {
        //                 fail!("Invalid asset source file path: {}", source.display());
        //             }
        //             Some(source) => {
        //                 let ext = match source.extension() {
        //                     None => None,
        //                     Some(ext) => ext.to_str(),
        //                 };

        //                 let mut selected_importers = Vec::new();
        //                 for (id, importer) in self.importers.iter() {
        //                     if let Some(ext) = ext {
        //                         if importer.extensions.iter().all(|e| **e != *ext) {
        //                             continue;
        //                         }
        //                     }

        //                     selected_importers.push((*id, importer));
        //                 }

        //                 if selected_importers.is_empty() {
        //                     tracing::warn!(
        //                         "No importers found for asset source file path '{}'",
        //                         source.display()
        //                     );
        //                 }

        //                 if selected_importers.len() != 1 {
        //                     tracing::info!(
        //                         "Multiple importers found for asset source file path '{}': {:?}",
        //                         source.display(),
        //                         selected_importers
        //                     );
        //                 }

        //                 let config = ImportConfig {
        //                     source: source,
        //                     importer: selected_importers.first().map(|(id, desc)| ImporterConfig {
        //                         id: *id,
        //                         value: desc.config.1.clone(),
        //                     }),
        //                 };

        //                 self.config.open(config);
        //             }
        //         };
        //     }
        // }

        // self.config.show(ui.ctx(), &self.importers);

        // egui::Frame::menu(ui.style()).show(ui, |ui| {
        //     ui.horizontal(|ui| {
        //         if ui.button("Import").clicked() {
        //             self.pick_asset.open(project);
        //         }
        //     });
        // });

        // egui::Frame::group(ui.style()).show(ui, |ui| {
        //     ui.horizontal(|ui| {
        //         ui.label("Target");

        //         egui::TextEdit::singleline(&mut self.lookup.target)
        //             .hint_text("Type of asset to look for")
        //             .desired_width(200.0)
        //             .show(ui);

        //         ui.label("Path");

        //         egui::TextEdit::singleline(&mut self.lookup.path)
        //             .hint_text("Base path to look for assets")
        //             .desired_width(200.0)
        //             .show(ui);
        //     });
        // });

        // egui::Frame::group(ui.style()).show(ui, |ui| {
        //     let target = match &*self.lookup.target {
        //         "" => None,
        //         target => match Ident::from_str(target) {
        //             Ok(target) => Some(target),
        //             Err(_) => {
        //                 ui.label("Invalid target");
        //                 return;
        //             }
        //         },
        //     };

        //     let path = match &*self.lookup.path {
        //         "" => None,
        //         path => Some(path),
        //     };

        //     let assets = self.store.select(target, path);

        //     ui.vertical(|ui| {
        //         for (id, asset) in assets {
        //             ui.horizontal(|ui| {
        //                 ui.label(id.to_string());
        //             });
        //         }
        //     });
        // });

        Ok(())
    }
}

struct PickAsset {
    file: Option<FileDialog>,
    selected: SmallVec<[PathBuf; 2]>,
}

impl PickAsset {
    fn new() -> Self {
        PickAsset {
            file: None,
            selected: SmallVec::new(),
        }
    }

    fn open(&mut self, project: &Project) {
        self.file.get_or_insert_with(|| {
            let path = project.root_path().join("assets");
            let mut file = FileDialog::open_file(Some(path))
                .show_drives(false)
                .title("Pick Asset");
            file.open();
            file
        });
    }

    fn show(&mut self, cx: &egui::Context, project: &Project) {
        if let Some(file) = &mut self.file {
            file.show(cx);

            let root = project.root_path().join("assets");

            match file.state() {
                egui_file::State::Open => {
                    if !file.directory().starts_with(&root) {
                        file.set_path(root);
                    }
                }
                egui_file::State::Closed => self.file = None,
                egui_file::State::Cancelled => self.file = None,
                egui_file::State::Selected => {
                    let path = file
                        .path()
                        .expect("Path should be selected when state is `Selected`");

                    if path.starts_with(&root) {
                        self.selected.push(path.to_path_buf());
                        self.file = None;
                    } else {
                        file.open();
                        file.set_path(root);
                    }
                }
            }
        }
    }

    fn take_selected(&mut self) -> Option<PathBuf> {
        self.selected.pop()
    }
}

struct ImportConfig {
    source: PathBuf,
    importer: Option<ImporterConfig>,
}

struct ImporterConfig {
    id: ImporterId,
    value: Value,
}

struct Configure {
    config: Option<ImportConfig>,
    configured: SmallVec<[ImportConfig; 2]>,
}

impl Configure {
    fn new() -> Self {
        Configure {
            config: None,
            configured: SmallVec::new(),
        }
    }

    fn open(&mut self, config: ImportConfig) {
        self.config = Some(config);
    }

    fn is_open(&self) -> bool {
        self.config.is_some()
    }

    fn show(&mut self, cx: &egui::Context, importers: &HashMap<ImporterId, ImporterDesc>) {
        if self.config.is_some() {
            let id = egui::Id::new("config_dialog");

            egui::Modal::new(id).show(cx, |ui| {
                let Some(config) = self.config.as_mut() else {
                    return;
                };

                ui.heading("Import Configuration");

                ui.horizontal(|ui| {
                    ui.label("Source:");
                    let source_text = config.source.display().to_string();
                    ui.text_edit_singleline(&mut &*source_text);
                });

                let mut selected_id = None;
                let mut cbox = egui::ComboBox::new("arcana-ed-select-importer", "Importer");

                if let Some(i) = &config.importer {
                    if let Some(desc) = importers.get(&i.id) {
                        selected_id = Some(i.id);
                        cbox = cbox.selected_text(desc.name.to_string());
                    } else {
                        config.importer = None;
                    }
                }

                cbox.show_ui(ui, |ui| {
                    for (id, desc) in importers {
                        ui.selectable_value(
                            &mut selected_id,
                            Some(*id),
                            format!("{} ({})", desc.name, desc.target),
                        );
                    }
                });

                match (&config.importer, selected_id) {
                    (None, None) => {}
                    (Some(i), Some(id)) if i.id == id => {}
                    (Some(_), None) => {
                        config.importer = None;
                    }
                    (_, Some(id)) => {
                        if let Some(desc) = importers.get(&id) {
                            config.importer = Some(ImporterConfig {
                                id,
                                value: desc.config.1.clone(),
                            });
                        }
                    }
                }

                if let Some(i) = &mut config.importer {
                    if let Some(desc) = importers.get(&i.id) {
                        ui.label(format!("Target: {}", desc.target));

                        ui.label(format!("Importer: {}. #{}", desc.name, i.id));

                        if ui.small_button("Reset to Default").clicked() {
                            i.value = desc.config.1.clone();
                        }

                        let probe = ValueProbe::new(
                            Some(&desc.config.0),
                            &mut i.value,
                            "arcana-ed-importer-config-value",
                        );

                        probe.show(ui);
                    } else {
                        config.importer = None;
                    }
                } else {
                    ui.label("Select an importer to configure");
                }

                let has_importer = config.importer.is_some();

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(has_importer, egui::Button::new("Ok"))
                        .clicked()
                    {
                        self.configured.extend(self.config.take());
                    }

                    if ui.button("Cancel").clicked() {
                        self.config = None;
                    }
                });
            });
        }
    }

    fn take_configured(&mut self) -> Option<ImportConfig> {
        self.configured.pop()
    }
}
