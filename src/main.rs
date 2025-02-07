#[macro_use]
extern crate rocket;
#[macro_use]
extern crate anyhow;
use rocket::fs::NamedFile;
use rocket::State;
use std::env;
use std::path;
#[macro_use]
mod approx;
mod slope;
mod srtm;
mod tile;

#[launch]
fn rocket() -> _ {
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    rocket::build()
        .manage(srtm::SRTM::new(path::Path::new(&cache).join("srtm")))
        .manage(slope::Pipeline::new(path::Path::new(&cache).join("slope")))
        .mount("/slope", routes![slope_tiles])
}

#[get("/<zoom>/<x>/<y_with_extension>")]
async fn slope_tiles(
    elev: &State<srtm::SRTM>,
    pipe: &State<slope::Pipeline>,
    zoom: u8,
    x: u32,
    y_with_extension: &str,
) -> Option<NamedFile> {
    if zoom < 10 || zoom > 14 {
        // todo support coarser zoom levels using coarser source data
        return None;
    }
    let y = y_with_extension
        .strip_suffix(".png")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let bounds = tile::tile_bounds(zoom, x, y);
    println!("tile bounds {:?}", bounds);
    let elevations = elev.get_all(bounds).await.unwrap();
    println!("elevations {:?}", elevations);
    let shade = pipe.get(&elevations, zoom, x, y).await.unwrap();
    NamedFile::open(&shade).await.ok()
}
