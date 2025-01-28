use std::{
    future::Future,
    path::Path,
    pin::Pin,
    task::{Context, Poll},
};

use arcana_names::Ident;
use arcana_project::real_path;
use egui::Ui;

mod store;

use egui_file::FileDialog;
use futures::future::BoxFuture;
use store::{Store, StoreInfo};
use url::Url;

use crate::{
    assets::{
        import::{EmptyConfig, ImportConfig, ImporterId},
        AssetData, AssetId, Error, Loader, NotFound,
    },
    task::{TaskQueue, WakerArray},
};

use super::instance::Instance;

struct AssetDataRequest {
    wakers: WakerArray,
    data: Option<AssetData>,
}

impl Future for AssetDataRequest {
    type Output = AssetData;

    fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
        self.get_mut().wakers.register(cx.waker());
        Poll::Pending
    }
}

pub struct AssetsLoader {
    task_queue: TaskQueue<AssetRequest, Result<Option<AssetData>, Error>>,
}

enum AssetRequest {
    Load { id: AssetId },
    Update { id: AssetId, version: u64 },
}

impl Loader for AssetsLoader {
    fn load<'a>(&'a self, id: AssetId) -> BoxFuture<'a, Result<AssetData, Error>> {
        let response = self.task_queue.push(AssetRequest::Load { id });
        Box::pin(async move {
            match response.await {
                Ok(None) => Err(Error::new(NotFound)), // Shouldn't happen for Load requests.
                Ok(Some(data)) => Ok(data),
                Err(e) => Err(e),
            }
        })
    }

    fn update<'a>(
        &'a self,
        id: AssetId,
        version: u64,
    ) -> BoxFuture<'a, Result<Option<AssetData>, Error>> {
        let response = self.task_queue.push(AssetRequest::Update { id, version });
        Box::pin(response)
    }
}

struct Lookup {
    // Type of asset to look for.
    target: String,

    // Base path to look for assets.
    path: String,
}

/// Assets viewer.
pub struct Assets {
    store: Store,

    // Where to look for assets.
    lookup: Lookup,

    import_dialog: Option<ImportDialog>,
}

enum ImportDialog {
    File(FileDialog),
    PickImporter {
        source_url: Url,
        importers: Vec<ImporterId>,
    },
    Config {
        source_url: Url,
        importer: ImporterId,
        config: Box<dyn ImportConfig>,
    },
    Error(ErrorDialog),
}

struct ErrorDialog {
    title: String,
    message: String,
}

impl Assets {
    pub fn new(base: &Path) -> Self {
        let store = Store::new(base, StoreInfo::default()).expect("Failed to create asset store");
        Assets {
            store,
            lookup: Lookup {
                target: String::new(),
                path: String::new(),
            },
            import_dialog: None,
        }
    }

    pub fn show(&mut self, ui: &mut Ui, instance: &Instance) {
        match self.import_dialog {
            Some(ImportDialog::Error(ref err)) => {
                let mut close = false;

                egui::Window::new(&err.title).show(ui.ctx(), |ui| {
                    ui.label(&err.message);
                    if ui.button("Close").clicked() {
                        close = true;
                    }
                });

                if close {
                    self.import_dialog = None;
                }
            }
            Some(ImportDialog::File(ref mut dialog)) => {
                dialog.show(ui.ctx());

                match dialog.state() {
                    egui_file::State::Closed | egui_file::State::Cancelled => {
                        self.import_dialog = None;
                    }
                    egui_file::State::Open => {}
                    egui_file::State::Selected => {
                        let source = dialog.path().unwrap();

                        let res = real_path(source).and_then(|path| {
                            let url = Url::from_file_path(&path).ok()?;
                            Some((path, url))
                        });

                        match res {
                            None => {
                                self.import_dialog = Some(ImportDialog::Error(ErrorDialog {
                                    title: "Invalid path".to_string(),
                                    message: "Invalid path selected".to_string(),
                                }));
                            }
                            Some((source_path, source_url)) => {
                                let ext = match source_path.extension() {
                                    None => None,
                                    Some(ext) => ext.to_str(),
                                };

                                let mut selected_importers = Vec::new();
                                for (id, importer) in instance.hub().importers.iter() {
                                    if let Some(ext) = ext {
                                        let supported = importer.extensions();
                                        if supported.iter().all(|e| **e != *ext) {
                                            continue;
                                        }
                                    }

                                    selected_importers.push((*id, &**importer));
                                }

                                if selected_importers.is_empty() {
                                    self.import_dialog = Some(ImportDialog::Error(ErrorDialog {
                                        title: "No importer found".to_string(),
                                        message: "No importer found for the selected file"
                                            .to_string(),
                                    }));
                                }

                                if selected_importers.len() == 1 {
                                    let (id, importer) = selected_importers[0];

                                    let config = importer.config();
                                    if config.is::<EmptyConfig>() {
                                        let res = self.store.store_from_url(
                                            source_url,
                                            importer.target(),
                                            None,
                                            instance.hub(),
                                        );

                                        match res {
                                            Ok(_) => {}
                                            Err(err) => {
                                                self.import_dialog =
                                                    Some(ImportDialog::Error(ErrorDialog {
                                                        title: "Failed to import".to_string(),
                                                        message: format!(
                                                            "Failed to import asset: {}",
                                                            err
                                                        ),
                                                    }));
                                            }
                                        }
                                    } else {
                                        self.import_dialog = Some(ImportDialog::Config {
                                            source_url,
                                            importer: id,
                                            config,
                                        });
                                    }
                                } else {
                                    self.import_dialog = Some(ImportDialog::PickImporter {
                                        source_url,
                                        importers: selected_importers
                                            .iter()
                                            .map(|(id, _)| *id)
                                            .collect(),
                                    });
                                }
                            }
                        };
                    }
                }
            }
            Some(_) => unimplemented!(),
            None => {}
        }

        egui::Frame::menu(ui.style()).show(ui, |ui| {
            if self.import_dialog.is_some() {
                ui.disable();
            }

            ui.horizontal(|ui| {
                if ui.button("Import").clicked() {
                    let mut dialog = FileDialog::open_file(None);
                    dialog.open();
                    self.import_dialog = Some(ImportDialog::File(dialog));
                }
            });
        });

        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Target");

                egui::TextEdit::singleline(&mut self.lookup.target)
                    .hint_text("Type of asset to look for")
                    .desired_width(200.0)
                    .show(ui);

                ui.label("Path");

                egui::TextEdit::singleline(&mut self.lookup.path)
                    .hint_text("Base path to look for assets")
                    .desired_width(200.0)
                    .show(ui);
            });
        });

        egui::Frame::group(ui.style()).show(ui, |ui| {
            let target = match &*self.lookup.target {
                "" => None,
                target => match Ident::from_str(target) {
                    Ok(target) => Some(target),
                    Err(_) => {
                        ui.label("Invalid target");
                        return;
                    }
                },
            };

            let path = match &*self.lookup.path {
                "" => None,
                path => Some(path),
            };

            let assets = self.store.select(target, path);

            ui.vertical(|ui| {
                for (id, asset) in assets {
                    ui.horizontal(|ui| {
                        ui.label(id.to_string());
                    });
                }
            });
        });
    }

    pub fn tick(&mut self, instance: &Instance) {}
}
