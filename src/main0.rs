mod lib;
use lib::{Config, other};

fn main() {
    other(
        "../shapez-community-edition/res_raw",
        "/tmp/atlas0",
        "atlas0".to_owned(),
        Config {
            padding_x: 2,
            padding_y: 2,
            max_width: 2048,
            max_height: 2048,
            scale: vec![0.25, 0.5, 0.75],
            scale_suffix: vec!["_lq".to_owned(), "_mq".to_owned(), "_hq".to_owned()],
        },
    )
    .unwrap();
}
