use crate::{
    model::ValueProbe,
    tool::{Tool, ToolContext, ToolTemplate},
};
use arcana::{
    assets::import::ImporterId,
    error::Error,
    model::{Model, Value},
};
use egui_file::FileDialog;
use std::path::PathBuf;
use winit::window::WindowId;

fn editable_import_config(model: &Model, default: &Value, stored: &Value) -> Value {
    if stored == &Value::Unit && matches!(model, Model::Record(_)) {
        default.clone()
    } else {
        stored.clone()
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct BrowserState {
    folder: PathBuf,
    query: String,
    fuzzy: bool,
    recursive: bool,
    tags: String,
    selection: Option<PathBuf>,
}

struct ImportConfig {
    source: PathBuf,
    importer: Option<ImporterId>,
    value: Value,
    model: Option<Model>,
    default_value: Option<Value>,
}

pub struct AssetBrowser {
    state: BrowserState,
    picker: Option<FileDialog>,
    import: Option<ImportConfig>,
    tag_edit: String,
    loaded_selection: Option<PathBuf>,
    status: String,
    cache_key: Option<(u64, PathBuf, String, bool, bool, String)>,
    results: Vec<usize>,
    id: egui::Id,
}

impl AssetBrowser {
    fn new(state: BrowserState) -> Self {
        Self {
            state,
            picker: None,
            import: None,
            tag_edit: String::new(),
            loaded_selection: None,
            status: String::new(),
            cache_key: None,
            results: Vec::new(),
            id: egui::Id::new(("asset-browser", rand::random::<u64>())),
        }
    }

    fn show(&mut self, ui: &mut egui::Ui, context: &mut ToolContext<'_>) -> Result<(), Error> {
        // Restored state is validated against the current catalog, without filesystem access per frame.
        if !self.state.folder.as_os_str().is_empty()
            && !context
                .assets
                .database()
                .entries()
                .iter()
                .any(|entry| entry.directory && entry.path == self.state.folder)
        {
            self.state.folder.clear();
        }
        ui.horizontal(|ui| {
            if ui.button("Import file…").clicked() {
                let mut dialog = FileDialog::open_file()
                    .id(self.id.with("picker"))
                    .initial_path(context.assets.base().join(&self.state.folder))
                    .show_drives(true)
                    .title("Import asset source");
                dialog.open();
                self.picker = Some(dialog);
            }
            if ui.button("Refresh").clicked() {
                match context.assets.refresh() {
                    Ok(()) => self.status = "Catalog refreshed".into(),
                    Err(error) => context.errors.push_error(
                        ui.ctx().viewport_id(),
                        "Asset refresh failed",
                        error,
                    ),
                }
            }
            ui.label(format!(
                "{} sources and folders",
                context.assets.database().entries().len()
            ));
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("assets").clicked() {
                self.state.folder.clear();
            }
            let mut path = PathBuf::new();
            for component in self.state.folder.clone().components() {
                path.push(component.as_os_str());
                ui.label("/");
                if ui.button(component.as_os_str().to_string_lossy()).clicked() {
                    self.state.folder = path.clone();
                }
            }
            if !self.state.folder.as_os_str().is_empty() && ui.small_button("Up").clicked() {
                self.state.folder.pop();
            }
        });
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.state.query).hint_text("Search asset names"),
            );
            ui.checkbox(&mut self.state.fuzzy, "Fuzzy");
            ui.checkbox(&mut self.state.recursive, "Subfolders");
        });
        ui.horizontal(|ui| {
            ui.label("Tags (all):");
            ui.add(
                egui::TextEdit::singleline(&mut self.state.tags).hint_text("Comma-separated tags"),
            );
        });
        let database = context.assets.database();
        let key = (
            database.revision(),
            self.state.folder.clone(),
            self.state.query.clone(),
            self.state.fuzzy,
            self.state.recursive,
            self.state.tags.clone(),
        );
        if self.cache_key.as_ref() != Some(&key) {
            self.results = database.search(
                &self.state.folder,
                &self.state.query,
                self.state.fuzzy,
                self.state.recursive,
                &parse_tags(&self.state.tags),
            );
            self.cache_key = Some(key);
        }
        if !database.warnings().is_empty() {
            ui.collapsing(
                format!("{} catalog warnings", database.warnings().len()),
                |ui| {
                    for warning in database.warnings() {
                        ui.colored_label(egui::Color32::YELLOW, warning);
                    }
                },
            );
        }
        ui.label(format!("{} results", self.results.len()));
        let mut selected = None;
        let mut folder = None;
        let height = ui.available_height().max(120.0).min(320.0);
        egui::ScrollArea::vertical()
            .id_salt("asset-results")
            .max_height(height)
            .show_rows(
                ui,
                ui.text_style_height(&egui::TextStyle::Body) + 8.0,
                self.results.len(),
                |ui, range| {
                    for row in range {
                        let entry = &database.entries()[self.results[row]];
                        let status = if entry.directory {
                            "Folder"
                        } else if entry.warning.is_some() {
                            "Metadata error"
                        } else if entry.imported.is_empty() {
                            "Source"
                        } else {
                            "Imported"
                        };
                        let label = format!(
                            "{}   [{}{}]   {}",
                            entry.path.display(),
                            status,
                            if entry.path.is_absolute() {
                                ", External"
                            } else {
                                ""
                            },
                            entry.tags.join(", ")
                        );
                        let response = ui.selectable_label(
                            self.state.selection.as_ref() == Some(&entry.path),
                            label,
                        );
                        if response.clicked() {
                            selected = Some(entry.path.clone());
                        }
                        if response.double_clicked() && entry.directory {
                            folder = Some(entry.path.clone());
                        }
                    }
                },
            );
        if let Some(path) = selected {
            self.tag_edit = database
                .entries()
                .iter()
                .find(|entry| entry.path == path)
                .map(|entry| entry.tags.join(", "))
                .unwrap_or_default();
            self.state.selection = Some(path);
            self.loaded_selection = self.state.selection.clone();
        }
        if let Some(path) = folder {
            self.state.folder = path;
        }
        if let Some(path) = self.state.selection.clone() {
            if let Some(entry) = context
                .assets
                .database()
                .entries()
                .iter()
                .find(|entry| entry.path == path)
            {
                ui.separator();
                ui.label(if path.is_absolute() {
                    format!("Selected external source: {}", path.display())
                } else {
                    format!("Selected: assets/{}", path.display())
                });
                if entry.directory {
                    if ui.button("Open folder").clicked() {
                        self.state.folder = path;
                    }
                } else {
                    if self.loaded_selection.as_ref() != Some(&path) {
                        self.tag_edit = entry.tags.join(", ");
                        self.loaded_selection = Some(path.clone());
                    }
                    let warning = entry.warning.clone();
                    let metadata_error = entry.metadata_error;
                    let has_imports = !entry.imported.is_empty();
                    for asset in &entry.imported {
                        ui.label(format!(
                            "{} • {} • version {}",
                            asset.target, asset.id, asset.version
                        ));
                    }
                    let supported = matching_importers(context.assets, &path).next().is_some();
                    if !supported {
                        ui.weak("No active plugin importer supports this extension.");
                    }
                    let mut import_clicked = false;
                    let mut deimport_clicked = false;
                    ui.horizontal(|ui| {
                        ui.label("Tags:");
                        ui.text_edit_singleline(&mut self.tag_edit);
                        if ui
                            .add_enabled(!metadata_error, egui::Button::new("Save tags"))
                            .clicked()
                        {
                            match context
                                .assets
                                .database_mut()
                                .set_tags(&path, parse_tags(&self.tag_edit))
                            {
                                Ok(()) => self.status = "Tags saved".into(),
                                Err(error) => context.errors.push_error(
                                    ui.ctx().viewport_id(),
                                    "Cannot save asset tags",
                                    error,
                                ),
                            }
                        }
                        import_clicked = ui
                            .add_enabled(
                                supported && !metadata_error,
                                egui::Button::new("Import / reimport"),
                            )
                            .clicked();
                        deimport_clicked = ui
                            .add_enabled(!metadata_error && has_imports, egui::Button::new("De-import"))
                            .on_hover_text("Remove all imported targets for this source. Keep source files and tags.")
                            .clicked();
                    });
                    if let Some(warning) = warning {
                        ui.colored_label(egui::Color32::YELLOW, warning);
                    }
                    if import_clicked {
                        self.begin_import(
                            context.assets.base().join(&path),
                            context.assets.database(),
                        )?;
                    }
                    if deimport_clicked {
                        match context.assets.deimport(&path) {
                            Ok(()) => {
                                if self.import.as_ref().is_some_and(|config| {
                                    config.source == context.assets.base().join(&path)
                                }) {
                                    self.import = None;
                                }
                                self.status = "Asset de-imported".into();
                            }
                            Err(error) => context.errors.push_error(
                                ui.ctx().viewport_id(),
                                "Cannot de-import asset",
                                error,
                            ),
                        }
                    }
                }
            }
        }
        if !self.status.is_empty() {
            ui.label(&self.status);
        }
        if let Some(picker) = &mut self.picker {
            picker.show(ui.ctx());
            match picker.state() {
                egui_file::State::Selected => {
                    let selected = picker.path().map(ToOwned::to_owned);
                    self.picker = None;
                    if let Some(path) = selected {
                        self.begin_import(path, context.assets.database())?;
                    }
                }
                egui_file::State::Cancelled | egui_file::State::Closed => self.picker = None,
                _ => {}
            }
        }
        self.import_dialog(ui.ctx(), context)?;
        Ok(())
    }

    fn begin_import(
        &mut self,
        source: PathBuf,
        database: &super::AssetDatabase,
    ) -> Result<(), Error> {
        let identity = database.import_source(&source)?;
        let source = if identity.is_absolute() {
            identity
        } else {
            database.resolve_source(&identity)?
        };
        self.import = Some(ImportConfig {
            source,
            importer: None,
            value: Value::default(),
            model: None,
            default_value: None,
        });
        Ok(())
    }

    fn import_dialog(
        &mut self,
        cx: &egui::Context,
        context: &mut ToolContext<'_>,
    ) -> Result<(), Error> {
        let Some(config) = &mut self.import else {
            return Ok(());
        };
        let mut confirmed = false;
        let mut cancelled = false;
        egui::Modal::new(self.id.with("import-config")).show(cx, |ui| {
            ui.heading("Import asset");
            ui.label(config.source.display().to_string());
            ui.weak(
                "Sources are imported in place. External source metadata is stored in assets/.external.",
            );
            let mut choices: Vec<_> = matching_importers(context.assets, &config.source).collect();
            choices.sort_by_key(|(id, desc)| (desc.target.to_string(), desc.name.to_string(), id.to_string()));
            if !choices.iter().any(|(id, _)| Some(*id) == config.importer) {
                config.importer = choices.first().map(|(id, _)| *id);
                config.model = None;
            }
            let previous = config.importer;
            let text = choices.iter().find(|(id,_)| Some(*id)==config.importer).map(|(_,desc)| format!("{} → {}", desc.name, desc.target)).unwrap_or_else(|| "No supported importer".into());
            egui::ComboBox::from_id_salt("asset-importer").selected_text(text).show_ui(ui, |ui| {
                for (id,desc) in &choices { ui.selectable_value(&mut config.importer, Some(*id), format!("{} → {}", desc.name, desc.target)); }
            });
            if previous != config.importer { config.model = None; }
            if let Some((_, desc)) = choices.iter().find(|(id,_)| Some(*id)==config.importer) {
                let initial = config.model.is_none();
                let reset = ui.small_button("Reset to default").clicked();
                if config.model.as_ref() != Some(&desc.config.0)
                    || config.default_value.as_ref() != Some(&desc.config.1)
                    || reset
                {
                    config.model = Some(desc.config.0.clone());
                    config.default_value = Some(desc.config.1.clone());
                    config.value = desc.config.1.clone();
                    if initial && !reset {
                        if let Ok(meta) = context.assets.database().open_meta(&config.source) {
                            if let Some(asset) = meta.get_asset(desc.target) {
                                if asset.importer == config.importer { config.value = editable_import_config(&desc.config.0, &desc.config.1, &asset.config); }
                            }
                        }
                    }
                }
                ValueProbe::new(Some(&desc.config.0), &mut config.value, "asset-import-value").show(ui);
            } else { ui.label("Enable a plugin with an importer for this file extension."); }
            ui.horizontal(|ui| {
                confirmed = ui.add_enabled(config.importer.is_some(), egui::Button::new("Import")).clicked();
                cancelled = ui.button("Cancel").clicked();
            });
        });
        if cancelled {
            self.import = None;
            return Ok(());
        }
        if confirmed {
            let config = self.import.as_mut().unwrap();
            let relative = context.assets.database().import_source(&config.source)?;
            let source = relative
                .to_str()
                .ok_or_else(|| Error::msg("Importer requires a UTF-8 source path".into()))?;
            let imported = context.assets.store(
                context.simulation.hub(),
                source,
                config.importer.unwrap(),
                config.value.clone(),
            );
            let refreshed = context.assets.refresh();
            let id = imported?;
            refreshed?;
            self.state.selection = Some(relative);
            self.status = format!("Imported asset {}", id);
            self.import = None;
        }
        Ok(())
    }
}

fn parse_tags(text: &str) -> Vec<String> {
    text.split(',')
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_owned)
        .collect()
}

fn matching_importers<'a>(
    store: &'a super::AssetStore,
    path: &std::path::Path,
) -> impl Iterator<Item = (ImporterId, &'a arcana::assets::import::ImporterDesc)> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_owned();
    store.importers().filter(move |(_, desc)| {
        desc.extensions
            .iter()
            .any(|supported| supported.eq_ignore_ascii_case(&extension))
    })
}

pub struct AssetsTemplate;

impl ToolTemplate for AssetsTemplate {
    fn glyph(&self) -> &str {
        egui_phosphor::regular::FOLDER_OPEN
    }

    fn description(&self) -> &str {
        "Browse, import, search, and tag project assets."
    }

    fn key(&self) -> &str {
        "assets"
    }
    fn title(&self) -> &str {
        "Assets"
    }
    fn create(&self, state: Option<serde_json::Value>) -> Result<Box<dyn Tool>, serde_json::Error> {
        let state = state
            .map(serde_json::from_value)
            .transpose()?
            .unwrap_or_default();
        Ok(Box::new(AssetBrowser::new(state)))
    }
}

impl Tool for AssetBrowser {
    fn title(&self) -> &str {
        "Assets"
    }
    fn ui(&mut self, ui: &mut egui::Ui, _: WindowId, context: &mut ToolContext<'_>) {
        if let Err(error) = self.show(ui, context) {
            context
                .errors
                .push_error(ui.ctx().viewport_id(), "Asset operation failed", error);
        }
    }
    fn save(&self) -> Option<serde_json::Value> {
        serde_json::to_value(&self.state).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_unit_configuration_uses_editable_record_defaults() {
        use arcana::assets::import::Importer;
        let desc = arcana::graphics::ImageImporter::desc();
        let value = editable_import_config(&desc.config.0, &desc.config.1, &Value::Unit);
        assert_eq!(value, desc.config.1);
        let decoded: arcana::graphics::ImageImportConfig =
            serde::Deserialize::deserialize(value).unwrap();
        assert_eq!(decoded, arcana::graphics::ImageImportConfig::default());
        assert_eq!(
            editable_import_config(&Model::Unit, &Value::Unit, &Value::Unit),
            Value::Unit
        );
        assert_eq!(
            editable_import_config(&desc.config.0, &desc.config.1, &Value::Bool(false)),
            Value::Bool(false)
        );
    }
    #[test]
    fn tool_state_roundtrip_and_template_registration() {
        let state = BrowserState {
            folder: "textures".into(),
            query: "brick".into(),
            fuzzy: true,
            recursive: true,
            tags: "stone,wall".into(),
            selection: Some("textures/Brick.png".into()),
        };
        let browser = AssetBrowser::new(state);
        let snapshot = browser.save().unwrap();
        let restored = AssetsTemplate.create(Some(snapshot.clone())).unwrap();
        assert_eq!(restored.title(), "Assets");
        assert_eq!(restored.save().unwrap(), snapshot);
        assert!(
            crate::tool::Toolbox::new()
                .materialize("assets", None)
                .is_some()
        );
        assert_eq!(parse_tags(" stone, ,wall "), ["stone", "wall"]);
    }
}
