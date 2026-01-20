use crunch::{Item, Rotation};
use image::{
    ImageReader, RgbaImage,
    imageops::{self, FilterType},
};
use std::{collections::BTreeMap, iter::zip, path::Path, time::Instant};
// use std::io::prelude::*;
use serde::{Deserialize, Serialize};
// use std::process::{Command, Output};
use walkdir::WalkDir;

// Use #[neon::export] to export Rust functions as JavaScript functions.
// See more at: https://docs.rs/neon/latest/neon/attr.export.html

#[neon::export]
fn hello(source: String, dest: String, name: String, config: String) {
    /*let hi = Command::new("java")
        .args([
            "-jar",
            "../gulp/runnable-texturepacker.jar",
            // format!("\"{source}\"").as_str(),
            // format!("\"{dest}\"").as_str(),
            // name.as_str(),
            // format!("\"{config}\"").as_str(),
            format!("{source}").as_str(),
            format!("{dest}").as_str(),
            name.as_str(),
            format!("{config}").as_str(),
        ])
        .output();
    // println!("{:?}", hi);
    match hi {
        Ok(Output {
            stdout,
            stderr,
            status,
        }) => {
            println!(
                "{}\n{}",
                String::from_utf8(stdout).unwrap(),
                String::from_utf8(stderr).unwrap()
            );
            "happy".to_string() + status.success().to_string().as_str()
        }
        Err(_) => "sad".to_string(),
    }*/
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
    dbg!(&config);
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
            // std::cmp::max(config.max_width, config.max_width) as usize,
            4096, items,
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
        /*for path in sources {
            let img = ImageReader::open(&path)?.decode()?;
            let img = if let Some(rgba8) = img.as_rgba8() {
                rgba8.to_owned()
            } else {
                img.to_rgba8()
            };
            let (trimmed, trimmed_loc_dims) = trim(&img);
            // println!("{img:?}");
            frames.insert(
                path.strip_prefix(&source)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_owned(),
                SpriteData {
                    frame: LocationDimensions {
                        x: 0,
                        y: 0,
                        w: trimmed_loc_dims.w,
                        h: trimmed_loc_dims.h,
                    },
                    rotated: false,
                    trimmed,
                    sprite_source_size: trimmed_loc_dims,
                    source_size: Dimensions {
                        w: img.width(),
                        h: img.height(),
                    },
                },
            );
        }*/
        let atlas_data = AtlasData { frames, meta };
        // println!("{}", serde_json::to_string_pretty(&atlas_data)?);
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
    // (
    //     false,
    //     LocationDimensions {
    //         x: 0,
    //         y: 0,
    //         w: img.width(),
    //         h: img.height(),
    //     },
    // )
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

// Use #[neon::main] to add additional behavior at module loading time.
// See more at: https://docs.rs/neon/latest/neon/attr.main.html

// #[neon::main]
// fn main(_cx: ModuleContext) -> NeonResult<()> {
//     println!("module is loaded!");
//     Ok(())
// }

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Config {
    _pot: bool,
    padding_x: u32,
    padding_y: u32,
    edge_padding: bool,
    _rotation: bool,
    max_width: u32,
    max_height: u32,
    _use_indexes: bool,
    _alpha_threshold: u8,
    strip_whitespace_x: bool,
    strip_whitespace_y: bool,
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

// "sprites/belt/built/forward_1.png": {
//   "frame": { "x": 512, "y": 476, "w": 116, "h": 144 },
//   "rotated": false,
//   "trimmed": true,
//   "spriteSourceSize": { "x": 14, "y": 0, "w": 116, "h": 144 },
//   "sourceSize": { "w": 144, "h": 144 }
// }

// {
//   "frames": {
//     "sprites/wires/wires_preview.png": {
//       "frame": { "x": 1205, "y": 19, "w": 48, "h": 48 },
//       "rotated": false,
//       "trimmed": true,
//       "spriteSourceSize": { "x": 0, "y": 0, "w": 48, "h": 48 },
//       "sourceSize": { "w": 48, "h": 48 }
//     }
//   },
//   "meta": {
//     "image": "atlas0_hq.png",
//     "format": "RGBA8888",
//     "size": { "w": 2048, "h": 2048 },
//     "scale": "0.75"
//   }
// }
