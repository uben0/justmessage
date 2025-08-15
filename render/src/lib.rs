use std::collections::HashMap;
use typst::{
    Library,
    diag::FileResult,
    foundations::{Bytes, Datetime},
    layout::{Abs, PagedDocument},
    syntax::{
        FileId, Source, VirtualPath,
        package::{PackageSpec, PackageVersion},
    },
    text::{Font, FontBook},
    utils::LazyHash,
};

pub struct Renderer {
    library: LazyHash<Library>,
    font_book: LazyHash<FontBook>,
    fonts: [Font; 1],
    map_sources: HashMap<FileId, Source>,
    map_bytes: HashMap<FileId, Bytes>,
}

pub struct Package {
    name: &'static str,
    version: PackageVersion,
    sources: &'static [(&'static str, &'static str)],
    bytes: &'static [(&'static str, &'static [u8])],
}

macro_rules! package {
    (
        $name:expr,
        ($major:expr, $minor:expr, $patch:expr),
        [$($path_source:literal),* $(,)?],
        [$($path_bytes:literal),* $(,)?]$(,)?
    ) => {
        Package {
            name: $name,
            version: PackageVersion {
                major: $major,
                minor: $minor,
                patch: $patch,
            },
            sources: &[$(($path_source, include_str!(concat!($name, "/", $path_source))),)*],
            bytes: &[$(($path_bytes, include_bytes!(concat!($name, "/", $path_bytes))),)*],
        }
    }
}

pub const OXIFMT: Package = package!(
    "oxifmt",
    (1, 0, 0),
    ["lib.typ", "oxifmt.typ"],
    ["typst.toml"],
);
pub const CETZ: Package = package!(
    "cetz",
    (0, 4, 1),
    [
        "src/lib.typ",
        "src/lib/palette.typ",
        "src/lib/angle.typ",
        "src/lib/tree.typ",
        "src/lib/decorations.typ",
        "src/lib/decorations/brace.typ",
        "src/lib/decorations/path.typ",
        "src/version.typ",
        "src/canvas.typ",
        "src/matrix.typ",
        "src/vector.typ",
        "src/wasm.typ",
        "src/util.typ",
        "src/deps.typ",
        "src/bezier.typ",
        "src/aabb.typ",
        "src/path-util.typ",
        "src/styles.typ",
        "src/process.typ",
        "src/drawable.typ",
        "src/coordinate.typ",
        "src/draw.typ",
        "src/draw/grouping.typ",
        "src/draw/transformations.typ",
        "src/draw/styling.typ",
        "src/draw/shapes.typ",
        "src/draw/projection.typ",
        "src/draw/util.typ",
        "src/intersection.typ",
        "src/anchor.typ",
        "src/hobby.typ",
        "src/complex.typ",
        "src/mark.typ",
        "src/mark-shapes.typ",
        "src/polygon.typ",
        "src/sorting.typ",
    ],
    ["typst.toml", "cetz-core/cetz_core.wasm"],
);

struct RendererWithFiles<'a> {
    main_id: FileId,
    main_source: Source,
    map_sources: HashMap<FileId, Source>,
    map_bytes: HashMap<FileId, Bytes>,
    renderer: &'a Renderer,
}

impl Renderer {
    pub fn attach_file(&mut self, path: &str, content: Bytes) {
        self.map_bytes
            .insert(FileId::new(None, VirtualPath::new(path)), content);
    }
    pub fn with_package(mut self, package: Package) -> Self {
        let package_spec = PackageSpec {
            namespace: "preview".into(),
            name: package.name.into(),
            version: package.version,
        };
        for (path, source) in package.sources {
            let id = FileId::new(Some(package_spec.clone()), VirtualPath::new(path));
            self.map_sources
                .insert(id, Source::new(id, source.to_string()));
        }
        for (path, bytes) in package.bytes {
            let id = FileId::new(Some(package_spec.clone()), VirtualPath::new(path));
            self.map_bytes.insert(id, Bytes::new(bytes));
        }
        self
    }
    pub fn new() -> Renderer {
        let fonts = [Font::new(Bytes::new(include_bytes!("FiraSans-Regular.otf")), 0).unwrap()];
        Self {
            library: LazyHash::new(Library::builder().build()),
            font_book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            map_sources: HashMap::new(),
            map_bytes: HashMap::new(),
        }
        .with_package(CETZ)
        .with_package(OXIFMT)
    }
    pub fn render(
        &self,
        main: &str,
        sources: HashMap<&str, String>,
        bytes: HashMap<&str, Vec<u8>>,
    ) -> Vec<u8> {
        let main_id = FileId::new_fake(VirtualPath::new("main.typ"));
        let result = typst::compile::<PagedDocument>(&RendererWithFiles {
            main_id,
            main_source: Source::new(main_id, main.into()),
            renderer: &self,
            map_sources: sources
                .into_iter()
                .map(|(path, source)| {
                    let file_id = FileId::new(None, VirtualPath::new(path));
                    (file_id, Source::new(file_id, source))
                })
                .collect(),
            map_bytes: bytes
                .into_iter()
                .map(|(path, bytes)| (FileId::new(None, VirtualPath::new(path)), Bytes::new(bytes)))
                .collect(),
        });
        let document = result.output.unwrap();
        typst_render::render_merged(&document, 2.0, Abs::mm(2.0), None)
            .encode_png()
            .unwrap()
    }
}

impl<'a> typst::World for RendererWithFiles<'a> {
    fn library(&self) -> &LazyHash<Library> {
        &self.renderer.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.renderer.font_book
    }

    fn main(&self) -> FileId {
        self.main_id
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main_id {
            Ok(self.main_source.clone())
        } else if let Some(source) = self.map_sources.get(&id) {
            Ok(source.clone())
        } else {
            match self.renderer.map_sources.get(&id) {
                Some(source) => Ok(source.clone()),
                None => {
                    panic!("huhe {:?}", id);
                }
            }
        }
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if let Some(bytes) = self.map_bytes.get(&id) {
            Ok(bytes.clone())
        } else {
            match self.renderer.map_bytes.get(&id) {
                Some(bytes) => Ok(bytes.clone()),
                None => {
                    panic!("haho {:?}", id);
                }
            }
        }
    }

    fn font(&self, index: usize) -> Option<Font> {
        match index {
            0 => Some(self.renderer.fonts[0].clone()),
            _ => None,
        }
    }

    fn today(&self, _offset: Option<i64>) -> Option<Datetime> {
        None
    }
}
