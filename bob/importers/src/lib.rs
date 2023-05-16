use std::{fmt::Display, fs::File, io::Write, path::Path};

use argosy_import::{Dependencies, ImportError, Importer, Sources};
use asefile::AsepriteFile;

/// Imports sprites from Aseprite files.
struct AsepriteSpriteImporter;

impl Importer for AsepriteSpriteImporter {
    fn name(&self) -> &str {
        "aseprite"
    }

    fn formats(&self) -> &[&str] {
        &["aseprite"]
    }

    fn extensions(&self) -> &[&str] {
        &["ase", "aseprite"]
    }

    fn target(&self) -> &str {
        "qoi"
    }

    fn import_dyn(
        &self,
        source: &Path,
        output: &Path,
        _sources: &mut dyn Sources,
        _dependencies: &mut dyn Dependencies,
    ) -> Result<(), ImportError> {
        let ase = AsepriteFile::read_file(source).map_err(error_to_reason)?;
        let frame = ase.frame(0);
        let image = frame.image();

        let qoi = rapid_qoi::Qoi {
            width: image.width(),
            height: image.height(),
            colors: rapid_qoi::Colors::SrgbLinA,
        };

        let encoded = qoi.encode_alloc(image.as_raw()).map_err(error_to_reason)?;

        let mut outfile = File::create(output).map_err(error_to_reason)?;
        outfile.write_all(&encoded).map_err(error_to_reason)?;
        Ok(())
    }
}

fn error_to_reason<E: Display>(error: E) -> ImportError {
    ImportError::Other {
        reason: error.to_string(),
    }
}

argosy_import::make_argosy_importers_library! {
    &AsepriteSpriteImporter;
}
