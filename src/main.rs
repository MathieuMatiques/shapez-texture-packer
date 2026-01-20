mod lib;
use lib::other;

fn main() {
    other(
        "../res_raw".to_owned(),
        "/tmp/atlas".to_owned(),
        "atlas0".to_owned(),
        "../res_raw/atlas.json".to_owned(),
    )
    .unwrap();
}
