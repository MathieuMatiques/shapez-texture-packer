use crunch::{Item, Rotation};
use image::{
    ImageReader, RgbaImage,
    imageops::{self, FilterType},
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, iter::zip, path::Path, time::Instant};
use walkdir::WalkDir;

#[neon::export]
fn hello(source: String, dest: String, name: String, config: String) {
    other(source, dest, name, config).unwrap()
}

pub fn other(
    source: String,
    dest: String,
    name: String,
    config: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let config: Config = serde_json::from_str(&std::fs::read_to_string(&config)?)?;
    // dbg!(&config);
    let sources = WalkDir::new(&source).into_iter().filter_map(|entry| {
        if let Ok(e) = entry
            && e.file_type().is_file()
            && e.path().extension()?.to_str()? == "png"
        {
            Some(e.into_path())
        } else {
            None
        }
        // entry.ok().and_then(|e| )
    });
    let loaded_imgs: BTreeMap<String, (RgbaImage, bool, LocationDimensions)> = sources
        .map(|path| {
            let img = ImageReader::open(&path).unwrap().decode().unwrap();
            let img = if let Some(rgba8) = img.as_rgba8() {
                rgba8.to_owned()
            } else {
                img.to_rgba8()
            };
            let (trimmed, trimmed_loc_dims) = trim(&img);
            let key = path
                .strip_prefix(&source)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            (key, (img, trimmed, trimmed_loc_dims))
        })
        .collect();

    for (scale, scale_suffix) in zip(config.scale, config.scale_suffix) {
        let items = loaded_imgs
            .iter()
            .map(|(key, (_, _, LocationDimensions { w, h, .. }))| {
                Item::new(
                    key.clone(),
                    (*w as f64 * scale).round() as usize + config.padding_x as usize,
                    (*h as f64 * scale).round() as usize + config.padding_y as usize,
                    Rotation::None,
                )
            });
        let packed = crunch::pack_into_po2(
            std::cmp::max(config.max_width, config.max_height) as usize,
            items,
        )
        .unwrap();

        let mut output = RgbaImage::new(packed.w as u32, packed.h as u32);

        let meta = MetaData {
            image: name.clone() + scale_suffix.as_str() + ".png",
            format: "RGBA8888".to_owned(),
            size: Dimensions {
                w: packed.w as u32,
                h: packed.h as u32,
            },
            scale: scale.to_string(),
        };
        let mut frames = BTreeMap::new();
        for item in packed.items.into_iter() {
            let (img, trimmed, trimmed_loc_dims) = loaded_imgs[&item.data].clone();
            frames.insert(
                item.data,
                SpriteData {
                    frame: LocationDimensions {
                        x: item.rect.x as u32 + config.padding_x / 2,
                        y: item.rect.y as u32 + config.padding_y / 2,
                        w: (trimmed_loc_dims.w as f64 * scale).round() as u32,
                        h: (trimmed_loc_dims.h as f64 * scale).round() as u32,
                    },
                    rotated: false,
                    trimmed,
                    sprite_source_size: LocationDimensions {
                        x: (trimmed_loc_dims.x as f64 * scale).round() as u32,
                        y: (trimmed_loc_dims.y as f64 * scale).round() as u32,
                        w: (trimmed_loc_dims.w as f64 * scale).round() as u32,
                        h: (trimmed_loc_dims.h as f64 * scale).round() as u32,
                    },
                    source_size: Dimensions {
                        w: (img.width() as f64 * scale).round() as u32,
                        h: (img.height() as f64 * scale).round() as u32,
                    },
                },
            );

            let cropped = imageops::crop_imm(
                &img,
                trimmed_loc_dims.x,
                trimmed_loc_dims.y,
                trimmed_loc_dims.w,
                trimmed_loc_dims.h,
            );

            let downsized = imageops::resize(
                &cropped.to_image(),
                (trimmed_loc_dims.w as f64 * scale).round() as u32,
                (trimmed_loc_dims.h as f64 * scale).round() as u32,
                FilterType::Triangle,
            );

            imageops::replace(
                &mut output,
                &downsized,
                item.rect.x as i64 + (config.padding_x / 2) as i64,
                item.rect.y as i64 + (config.padding_y / 2) as i64,
            );
        }
        let atlas_data = AtlasData { frames, meta };
        std::fs::write(
            Path::new(&dest).join(name.clone() + scale_suffix.as_str() + ".json"),
            serde_json::to_string(&atlas_data)?,
        )?;
        output.save(Path::new(&dest).join(name.clone() + scale_suffix.as_str() + ".png"))?;
    }
    let end = Instant::now();
    println!("{:?}", end - start);

    Ok(())
}

fn trim(img: &RgbaImage) -> (bool, LocationDimensions) {
    let (width, height) = img.dimensions();
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut found_pixel = false;

    // 1. Scan for the bounding box of non-transparent pixels
    for y in 0..height {
        for x in 0..width {
            let pixel = img.get_pixel(x, y);

            // Check alpha channel (index 3). 0 is fully transparent.
            if pixel.0[3] > 0 {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
                found_pixel = true;
            }
        }
    }

    // 2. Handle fully transparent images (return a 1x1 empty view at 0,0)
    if !found_pixel {
        panic!("all transparent!");
    }

    // 3. Calculate new dimensions
    let new_width = max_x - min_x + 1;
    let new_height = max_y - min_y + 1;
    (
        width != new_width || height != new_height,
        LocationDimensions {
            x: min_x,
            y: min_y,
            w: new_width,
            h: new_height,
        },
    )
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Config {
    _pot: bool,
    padding_x: u32,
    padding_y: u32,
    _edge_padding: bool,
    _rotation: bool,
    max_width: u32,
    max_height: u32,
    _use_indexes: bool,
    _alpha_threshold: u8,
    _strip_whitespace_x: bool,
    _strip_whitespace_y: bool,
    duplicate_padding: bool,
    _alias: bool,
    _fast: bool,
    _limit_memory: bool,
    _combine_subdirectories: bool,
    _flatten_paths: bool,
    _bleed_iterations: u32,
    scale: Vec<f64>,
    scale_suffix: Vec<String>,
}

#[derive(Serialize, Debug)]
struct AtlasData {
    frames: BTreeMap<String, SpriteData>,
    meta: MetaData,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SpriteData {
    frame: LocationDimensions,
    rotated: bool,
    trimmed: bool,
    sprite_source_size: LocationDimensions,
    source_size: Dimensions,
}

#[derive(Serialize, Debug)]
struct MetaData {
    image: String,
    format: String,
    size: Dimensions,
    scale: String,
}

#[derive(Serialize, Debug, Clone)]
struct LocationDimensions {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(Serialize, Debug)]
struct Dimensions {
    w: u32,
    h: u32,
}
