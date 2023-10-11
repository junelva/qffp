use super::{path, AppError, Error};
use image::io::Reader as ImageReader;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct WH {
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Xywh {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpriteSheetJSONFrame {
    pub filename: String,
    pub frame: Xywh,
    pub rotated: bool,
    pub trimmed: bool,
    #[serde(rename = "spriteSourceSize")]
    pub sprite_source_size: Xywh,
    #[serde(rename = "sourceSize")]
    pub source_size: WH,
    pub duration: u32,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct SpriteSheetJSONMeta {
    pub app: String,
    pub version: String,
    pub image: String,
    pub format: String,
    pub size: WH,
    pub scale: String,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct SpriteSheetJSON {
    pub frames: Vec<SpriteSheetJSONFrame>,
    pub meta: SpriteSheetJSONMeta,
}

#[derive(Default, Debug)]
pub struct LoadedSprite {
    pub name: String,
    pub index: usize,
    pub data: SpriteSheetJSON,
    pub image: Vec<Vec<Rgba>>,
}

#[derive(Error)]
pub struct SpriteStoreError;

impl std::fmt::Display for SpriteStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SpriteStore Error")
    }
}
impl std::fmt::Debug for SpriteStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{{ file: {}, line: {} }}", file!(), line!())
    }
}

fn load_sprite(json_path: &str, index: usize) -> Result<LoadedSprite, AppError> {
    let test_path = path::Path::new(json_path);
    if !test_path.exists() {
        println!("current dir: {:?}", std::env::current_dir());
        println!("path does not exist: {:?}", test_path);
        return Err(AppError::SpriteStore(SpriteStoreError));
    }

    let json_str = std::fs::read_to_string(json_path).expect("file read to string error");
    let json = serde_json::from_str::<SpriteSheetJSON>(&json_str)?;

    let image_path = format!("res/sheets/{}", json.meta.image);
    let test_path = path::Path::new(image_path.as_str());
    if !test_path.exists() {
        println!("current dir: {:?}", std::env::current_dir());
        println!("path does not exist: {:?}", test_path);
        return Err(AppError::SpriteStore(SpriteStoreError));
    }

    let image = ImageReader::open(format!("res/sheets/{}", json.meta.image))?
        .decode()?
        .into_rgba8();

    // convert rows to indexable vec
    let rows = image.rows();
    let mut rows_vec: Vec<Vec<Rgba>> = vec![];
    for row in rows {
        let mut row_vec: Vec<Rgba> = vec![];
        for pixel in row {
            row_vec.push(Rgba {
                r: pixel[0],
                g: pixel[1],
                b: pixel[2],
                a: pixel[3],
            });
        }
        rows_vec.push(row_vec);
    }

    let name = json.meta.image.rsplit_once('.').unwrap().0.to_string();

    Ok(LoadedSprite {
        name,
        index,
        data: json,
        image: rows_vec,
    })
}

#[derive(Default)]
pub struct SpriteStore(pub Vec<LoadedSprite>);

impl SpriteStore {
    pub fn new(json_paths: Vec<&str>) -> Result<SpriteStore, AppError> {
        let mut store: SpriteStore = SpriteStore(Vec::new());
        for (index, path) in json_paths.iter().enumerate() {
            store.0.push(load_sprite(path, index)?);
        }

        Ok(store)
    }

    #[allow(dead_code)]
    pub fn by_index(&self, index: usize) -> Result<&LoadedSprite, SpriteStoreError> {
        if self.0.is_empty() || self.0.len() - 1 < index {
            return Err(SpriteStoreError);
        }
        Ok(&self.0[index])
    }

    #[allow(dead_code)]
    pub fn by_name(&self, name: String) -> Result<&LoadedSprite, SpriteStoreError> {
        for sprite in self.0.iter() {
            if sprite.name == name {
                return Ok(sprite);
            }
        }
        Err(SpriteStoreError)
    }

    pub fn index_by_name(&self, name: &str) -> Result<usize, SpriteStoreError> {
        for sprite in self.0.iter() {
            if sprite.name == name {
                return Ok(sprite.index);
            }
        }
        Err(SpriteStoreError)
    }
}
