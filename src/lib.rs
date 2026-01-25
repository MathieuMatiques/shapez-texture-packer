// #![feature(iterator_try_collect)]
use crunch::{Item, Rotation};
use image::{
    ImageReader, RgbaImage,
    imageops::{self, FilterType},
};
use nanoserde::SerJson;
// use neon::{
//     handle::Handle,
//     object::Object,
//     prelude::FunctionContext,
//     result::NeonResult,
//     types::{JsArray, JsNumber, JsObject, JsString, Value},
// };
use napi_derive::napi;
use rayon::prelude::*;
// use orx_parallel::*;
// use serde::Serialize;
use std::{collections::BTreeMap, path::Path};
use walkdir::WalkDir;

// #[neon::export]
#[napi]
fn hello(
    // cx: &mut FunctionContext,
    source: String,
    dest: String,
    name: String,
    // config: Handle<JsObject>,
    config: Config,
) {
    // let config = Config {
    //     padding_x: config.prop(cx, "paddingX").get::<f64>()? as u32,
    //     padding_y: config.prop(cx, "paddingY").get::<f64>()? as u32,
    //     max_width: config.prop(cx, "maxWidth").get::<f64>()? as u32,
    //     max_height: config.prop(cx, "maxHeight").get::<f64>()? as u32,
    //     scale: config
    //         .prop(cx, "scale")
    //         .get::<Handle<JsArray>>()?
    //         .to_vec(cx)?
    //         .iter()
    //         .map(|val| {
    //             val.downcast::<JsNumber, _>(cx)
    //                 .map(|string| string.value(cx))
    //         })
    //         .try_collect()
    //         .unwrap(),
    //     scale_suffix: config
    //         .prop(cx, "scaleSuffix")
    //         .get::<Handle<JsArray>>()?
    //         .to_vec(cx)?
    //         .iter()
    //         .map(|val| {
    //             val.downcast::<JsString, _>(cx)
    //                 .map(|string| string.value(cx))
    //         })
    //         .try_collect()
    //         .unwrap(),
    // };
    other(source, dest, name, config).unwrap();
    // Ok(())
}

pub fn other<P: AsRef<Path> + Sync>(
    source: P,
    dest: P,
    name: String,
    config: Config,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(&dest)?;

    // let start = Instant::now();

    // let config: Config = serde_json::from_str(&std::fs::read_to_string(&config)?)?;
    // dbg!(&config);
    let sources: Vec<_> = WalkDir::new(&source)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "png"))
        .map(|e| e.into_path())
        .collect();

    let loaded_imgs: BTreeMap<String, (RgbaImage, bool, LocationDimensions)> = sources
        .par_iter()
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

    std::iter::zip(config.scale, config.scale_suffix)
        .collect::<Vec<_>>()
        .into_par_iter()
        .try_for_each(|(scale, scale_suffix)| -> anyhow::Result<()> {
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
            .expect("Failed to pack");

            let processed_sprites: Vec<_> = packed
                .items
                .into_par_iter()
                .map(|item| {
                    let (ref img, trimmed, ref trimmed_loc_dims) = loaded_imgs[&item.data];

                    let scaled_trimmed_loc_dims = LocationDimensions {
                        x: (trimmed_loc_dims.x as f64 * scale).round() as u32,
                        y: (trimmed_loc_dims.y as f64 * scale).round() as u32,
                        w: (trimmed_loc_dims.w as f64 * scale).round() as u32,
                        h: (trimmed_loc_dims.h as f64 * scale).round() as u32,
                    };

                    let sprite_data = SpriteData {
                        frame: LocationDimensions {
                            x: item.rect.x as u32 + config.padding_x / 2,
                            y: item.rect.y as u32 + config.padding_y / 2,
                            w: scaled_trimmed_loc_dims.w,
                            h: scaled_trimmed_loc_dims.h,
                        },
                        rotated: false,
                        trimmed,
                        sprite_source_size: scaled_trimmed_loc_dims.clone(),
                        source_size: Dimensions {
                            w: (img.width() as f64 * scale).round() as u32,
                            h: (img.height() as f64 * scale).round() as u32,
                        },
                    };

                    let cropped = imageops::crop_imm(
                        img,
                        trimmed_loc_dims.x,
                        trimmed_loc_dims.y,
                        trimmed_loc_dims.w,
                        trimmed_loc_dims.h,
                    );

                    let downsized = imageops::resize(
                        &cropped.to_image(),
                        scaled_trimmed_loc_dims.w,
                        scaled_trimmed_loc_dims.h,
                        FilterType::Triangle,
                    );

                    (item.data, sprite_data, downsized)
                })
                .collect();

            let mut output = RgbaImage::new(packed.w as u32, packed.h as u32);

            let mut frames = BTreeMap::new();

            for (key, sprite_data, downsized) in processed_sprites {
                imageops::replace(
                    &mut output,
                    &downsized,
                    sprite_data.frame.x as i64,
                    sprite_data.frame.y as i64,
                );
                frames.insert(key, sprite_data);
            }

            let meta = MetaData {
                image: name.clone() + scale_suffix.as_str() + ".png",
                format: "RGBA8888".to_owned(),
                size: Dimensions {
                    w: packed.w as u32,
                    h: packed.h as u32,
                },
                scale: scale.to_string(),
            };

            let atlas_data = AtlasData { frames, meta };
            std::fs::write(
                dest.as_ref()
                    .join(name.clone() + scale_suffix.as_str() + ".json"),
                // serde_json::to_string(&atlas_data)?,
                atlas_data.serialize_json(),
            )?;
            output.save(
                dest.as_ref()
                    .join(name.clone() + scale_suffix.as_str() + ".png"),
            )?;
            Ok(())
        })?;
    // let end = Instant::now();
    // println!("{:?}", end - start);

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

#[derive(Debug)]
#[napi(object)]
// #[serde(rename_all = "camelCase")]
pub struct Config {
    // _pot: bool,
    pub padding_x: u32,
    pub padding_y: u32,
    // _edge_padding: bool,
    // _rotation: bool,
    pub max_width: u32,
    pub max_height: u32,
    // _use_indexes: bool,
    // _alpha_threshold: u8,
    // _strip_whitespace_x: bool,
    // _strip_whitespace_y: bool,
    // duplicate_padding: bool,
    // _alias: bool,
    // _fast: bool,
    // _limit_memory: bool,
    // _combine_subdirectories: bool,
    // _flatten_paths: bool,
    // _bleed_iterations: u32,
    pub scale: Vec<f64>,
    pub scale_suffix: Vec<String>,
}

#[derive(SerJson, Debug)]
struct AtlasData {
    frames: BTreeMap<String, SpriteData>,
    meta: MetaData,
}

#[derive(SerJson, Debug)]
// #[serde(rename_all = "camelCase")]
struct SpriteData {
    frame: LocationDimensions,
    rotated: bool,
    trimmed: bool,
    #[nserde(rename = "spriteSourceSize")]
    sprite_source_size: LocationDimensions,
    #[nserde(rename = "sourceSize")]
    source_size: Dimensions,
}

#[derive(SerJson, Debug)]
struct MetaData {
    image: String,
    format: String,
    size: Dimensions,
    scale: String,
}

#[derive(SerJson, Debug, Clone)]
struct LocationDimensions {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

#[derive(SerJson, Debug)]
struct Dimensions {
    w: u32,
    h: u32,
}
