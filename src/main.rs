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
async fn slope_tiles(zoom: u8, x: u32, y_with_extension: &str) -> Option<NamedFile> {
    let y = y_with_extension
        .strip_suffix(".png")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    let geopoint = tile::square_to_geodetic(&tile::tile_to_square(zoom, x as f64, y as f64));
    println!("geopoint {:?}", geopoint);
    let shade = rocket::tokio::task::spawn_blocking(move || {
        let elevation = srtm::get(geopoint).unwrap();
        println!("elevation {:?}", elevation);
        let elevation_tile = tile::single_tile(elevation, zoom, x as f64, y as f64).unwrap();
        println!("elevation tile {:?}", elevation_tile);
        let slope = slope::slope(elevation_tile).unwrap();
        println!("slope {:?}", slope);
        let shade = slope::angle_shade(slope).unwrap();
        println!("shade {:?}", shade);
        shade
    })
    .await
    .ok()?;
    // let path = path::Path::new(&cache)
    //     .join("srtm_13_03_slope_angle_shade_tiles")
    //     .join(format!("{}", zoom))
    //     .join(format!("{}", x))
    //     .join(format!("{}.png", y));
    // println!("trying {:?}", path);
    NamedFile::open(&shade).await.ok()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index])
        .mount("/slope", routes![slope_tiles])
}
