#[macro_use]
extern crate rocket;
use flytile::sentinel;
use flytile::slope;
use flytile::srtm;
use flytile::tile;
use flytile::viewer;
use maud::Markup;
use rocket::fairing::Fairing;
use rocket::fairing::Info;
use rocket::fairing::Kind;
use rocket::fs::FileServer;
use rocket::fs::NamedFile;
use rocket::State;
use rocket::{Request, Response};
use std::borrow::Cow;
use std::env;
use std::path;

#[launch]
fn rocket() -> _ {
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    rocket::build()
        .attach(AnyOrigin)
        .manage(srtm::SRTM::new(path::Path::new(&cache).join("srtm")))
        .manage(slope::Pipeline::new(path::Path::new(&cache).join("slope")))
        .manage(sentinel::Sentinel::new(
            path::Path::new(&cache).join("sentinel"),
        ))
        .mount("/", routes![index])
        .mount("/css", FileServer::from("css"))
        .mount("/grid", routes![grid])
        .mount("/slope", routes![slope_tiles])
        .mount("/imagery/latest", routes![image_tiles])
}

struct AnyOrigin;

#[rocket::async_trait]
impl Fairing for AnyOrigin {
    fn info(&self) -> Info {
        Info {
            name: "Allow all origins",
            kind: Kind::Response,
        }
    }

    async fn on_response<'r>(&self, _request: &'r Request<'_>, response: &mut Response<'r>) {
        response.remove_header("X-Frame-Options");
    }
}

#[get("/")]
fn index() -> Markup {
    let point = tile::GeoPoint {
        longitude: -119.59018,
        latitude: 49.49230,
    };
    let zoom = 12;
    let square = tile::geodetic_to_square(&point);
    let point = tile::square_to_tile(zoom, &square);
    viewer::viewer(zoom, point.x as u32, point.y as u32)
}

#[get("/<zoom>/<x>/<y_with_extension>")]
fn grid(zoom: u8, x: u32, y_with_extension: &str) -> Markup {
    let y = y_with_extension
        .strip_suffix(".png")
        .unwrap()
        .parse::<u32>()
        .unwrap();
    // let point = tile::GeoPoint {
    //     longitude: -119.59018,
    //     latitude: 49.49230,
    // };
    // let zoom = 12;
    // let square = tile::geodetic_to_square(&point);
    // let point = tile::square_to_tile(zoom, &square);
    viewer::image_grid(zoom, x, y, 9, 5)
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
    log::info!("generating srtm slope tile {} {} {}", zoom, x, y);
    let bounds = tile::tile_bounds(zoom, x, y);
    log::debug!("tile bounds {:?}", bounds);
    let elevations = elev.get_all(bounds).await.unwrap();
    log::debug!("elevations {:?}", elevations);
    let shade = pipe.get(elevations, zoom, x, y).await.unwrap();
    NamedFile::open(&shade).await.ok()
}

#[get("/<zoom>/<x>/<y_with_extension>")]
async fn image_tiles(
    provider: &State<sentinel::Sentinel>,
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
    log::info!("generating sentinel imagery tile {} {} {}", zoom, x, y);
    let path = provider.get(zoom, x, y).await.unwrap();
    NamedFile::open(&path).await.ok()
}
