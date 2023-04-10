use std::{borrow::Cow, fmt};

use codespan_reporting::{
    diagnostic::{Diagnostic, Label},
    files::SimpleFile,
    term::{self, termcolor::Buffer},
};
use naga::FastHashMap;

use crate::backend::{CreateLibraryErrorKind, Library};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

impl fmt::Display for ShaderStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderStage::Vertex => write!(f, "vertex"),
            ShaderStage::Fragment => write!(f, "fragment"),
            ShaderStage::Compute => write!(f, "compute"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderLanguage {
    SpirV,
    Wgsl,
    Glsl { stage: ShaderStage },
    Msl,
}

impl fmt::Display for ShaderLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderLanguage::SpirV => write!(f, "SPIR-V"),
            ShaderLanguage::Wgsl => write!(f, "WGSL"),
            ShaderLanguage::Glsl { stage } => write!(f, "GLSL {}", stage),
            ShaderLanguage::Msl => write!(f, "MSL"),
        }
    }
}

pub struct ShaderSource<'a> {
    pub code: Cow<'a, [u8]>,
    pub filename: Option<&'a str>,
    pub language: ShaderLanguage,
}

pub enum LibraryInput<'a> {
    Source(ShaderSource<'a>),
}

pub struct LibraryDesc<'a> {
    pub name: &'a str,
    pub input: LibraryInput<'a>,
}

pub struct Shader<'a> {
    pub library: Library,
    pub entry: Cow<'a, str>,
}

#[derive(Debug)]
pub struct CreateLibraryError(pub(crate) CreateLibraryErrorKind);

impl fmt::Display for CreateLibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Debug)]
pub(crate) enum ShaderCompileError {
    NonUtf8(std::str::Utf8Error),
    ParseSpirV(naga::front::spv::Error),
    ParseWgsl(naga::front::wgsl::ParseError),
    ParseGlsl(Vec<naga::front::glsl::Error>),
    ValidationFailed,

    #[cfg(any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))))]
    GenSpirV(naga::back::spv::Error),

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    GenMsl(naga::back::msl::Error),
}

impl fmt::Display for ShaderCompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShaderCompileError::NonUtf8(err) => write!(f, "non-utf8: {}", err),
            ShaderCompileError::ParseSpirV(err) => write!(f, "parse SPIR-V: {}", err),
            ShaderCompileError::ParseWgsl(err) => write!(f, "parse WGSL: {}", err),
            ShaderCompileError::ParseGlsl(errs) => {
                write!(f, "parse GLSL: ")?;
                for err in errs {
                    write!(f, "{}", err)?;
                }
                Ok(())
            }
            ShaderCompileError::ValidationFailed => write!(f, "validation failed"),
            #[cfg(any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))))]
            ShaderCompileError::GenSpirV(err) => write!(f, "generate SPIR-V: {}", err),
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            ShaderCompileError::GenMsl(err) => write!(f, "generate MSL: {}", err),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) fn compile_shader(
    code: &[u8],
    filename: Option<&str>,
    lang: ShaderLanguage,
) -> Result<String, ShaderCompileError> {
    let (module, info) = parse_shader(code, filename, lang)?;

    let options = naga::back::msl::Options {
        lang_version: (2, 4),
        per_stage_map: Default::default(),
        inline_samplers: Vec::new(),
        spirv_cross_compatibility: false,
        fake_missing_bindings: false,
        bounds_check_policies: Default::default(),
        zero_initialize_workgroup_memory: false,
    };

    let (string, _translation) = naga::back::msl::write_string(
        &module,
        &info,
        &options,
        &naga::back::msl::PipelineOptions {
            allow_point_size: false,
        },
    )
    .map_err(ShaderCompileError::GenMsl)?;

    Ok(string)
}

#[cfg(any(windows, all(unix, not(any(target_os = "macos", target_os = "ios")))))]
pub(crate) fn compile_shader(
    code: &[u8],
    filename: Option<&str>,
    lang: ShaderLanguage,
) -> Result<Box<[u32]>, ShaderCompileError> {
    let (module, info) = parse_shader(code, filename, lang)?;

    let options = naga::back::spv::Options {
        lang_version: (1, 3),
        flags: naga::back::spv::WriterFlags::empty(),
        binding_map: naga::back::spv::BindingMap::default(),
        capabilities: None,
        bounds_check_policies: naga::proc::BoundsCheckPolicies::default(),
        zero_initialize_workgroup_memory: naga::back::spv::ZeroInitializeWorkgroupMemoryMode::None,
    };

    let words = naga::back::spv::write_vec(&module, &info, &options, None)
        .map(|vec| vec.into())
        .map_err(ShaderCompileError::GenSpirV)?;

    Ok(words)
}

pub(crate) fn parse_shader(
    code: &[u8],
    filename: Option<&str>,
    lang: ShaderLanguage,
) -> Result<(naga::Module, naga::valid::ModuleInfo), ShaderCompileError> {
    let module = match lang {
        ShaderLanguage::SpirV => {
            naga::front::spv::parse_u8_slice(code, &naga::front::spv::Options::default())
                .map_err(ShaderCompileError::ParseSpirV)?
        }
        ShaderLanguage::Msl => {
            unimplemented!("Compilation from MSL is not supported")
        }
        ShaderLanguage::Wgsl => {
            let code = std::str::from_utf8(code).map_err(ShaderCompileError::NonUtf8)?;
            naga::front::wgsl::parse_str(code).map_err(ShaderCompileError::ParseWgsl)?
        }
        ShaderLanguage::Glsl { stage } => {
            let code = std::str::from_utf8(code).map_err(ShaderCompileError::NonUtf8)?;
            naga::front::glsl::Parser::default()
                .parse(
                    &naga::front::glsl::Options {
                        defines: FastHashMap::default(),
                        stage: match stage {
                            ShaderStage::Vertex => naga::ShaderStage::Vertex,
                            ShaderStage::Fragment => naga::ShaderStage::Fragment,
                            ShaderStage::Compute => naga::ShaderStage::Compute,
                        },
                    },
                    code,
                )
                .map_err(ShaderCompileError::ParseGlsl)?
        }
    };

    let flags = naga::valid::ValidationFlags::all();
    let caps = naga::valid::Capabilities::empty();
    let info = naga::valid::Validator::new(flags, caps)
        .validate(&module)
        .map_err(|e| {
            emit_annotated_error(
                &e,
                filename.and_then(|filename| {
                    std::str::from_utf8(code)
                        .ok()
                        .map(|source| (filename, source))
                }),
            );
            ShaderCompileError::ValidationFailed
        })?;

    Ok((module, info))
}

fn emit_annotated_error<E: std::error::Error>(
    error: &naga::WithSpan<E>,
    file: Option<(&str, &str)>,
) {
    if let Some((filename, source)) = file {
        let files = SimpleFile::new(filename, source);
        let config = term::Config::default();
        let mut writer = Buffer::no_color();

        let diagnostic = Diagnostic::error().with_labels(
            error
                .spans()
                .map(|(span, desc)| {
                    Label::primary((), span.to_range().unwrap()).with_message(desc.to_owned())
                })
                .collect(),
        );

        term::emit(&mut writer, &config, &files, &diagnostic).expect("cannot write error");

        if let Ok(s) = std::str::from_utf8(writer.as_slice()) {
            tracing::event!(
                target: "naga",
                tracing::Level::ERROR,
                error = error.as_inner().to_string(),
                diagnostic = s,
            );
            return;
        }
    }

    tracing::event!(
        target: "naga",
        tracing::Level::ERROR,
        error = error.as_inner().to_string(),
    );
}
