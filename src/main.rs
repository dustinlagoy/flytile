#[macro_use]
extern crate rocket;
use rocket::fs::NamedFile;
use std::env;
use std::path;
#[macro_use]
mod approx;
mod slope;
mod srtm;
mod tile;

#[get("/")]
fn index() -> String {
    let path = srtm::download(13, 3).unwrap();
    format!("got {}", path.to_str().unwrap())
}

#[get("/<zoom>/<x>/<y_with_extension>")]
async fn slope_tiles(zoom: u8, x: u8, y_with_extension: &str) -> Option<NamedFile> {
    let y = y_with_extension
        .strip_suffix(".png")
        .unwrap()
        .parse::<u8>()
        .unwrap();
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    let geopoint = tile::tile_to_geodetic(
        zoom,
        &tile::Point {
            x: x as f64,
            y: y as f64,
        },
    );
    let elevation = srtm::get(geopoint);
    let path = path::Path::new(&cache)
        .join("srtm_13_03_slope_angle_shade_tiles")
        .join(format!("{}", zoom))
        .join(format!("{}", x))
        .join(format!("{}.png", y));
    println!("trying {:?}", path);
    NamedFile::open(&path).await.ok()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index])
        .mount("/slope", routes![slope_tiles])
}
