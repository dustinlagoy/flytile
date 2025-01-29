#[macro_use]
extern crate rocket;
use rocket::fs::NamedFile;
use std::path;
mod slope;
mod srtm;
mod tile;

#[get("/")]
fn index() -> String {
    let path = srtm::download(13, 3).unwrap();
    format!("got {}", path.to_str().unwrap())
}

#[get("/<zoom>/<x>/<y>")]
async fn slope_tiles(zoom: u8, x: u8, y: &str) -> Option<NamedFile> {
    let name = format!(
        "/tmp/srtm_13_03_slope_angle_shade_tiles/{}/{}/{}",
        zoom, x, y
    );
    println!("trying {}", name);
    NamedFile::open(path::Path::new(&name)).await.ok()
}

#[launch]
fn rocket() -> _ {
    rocket::build()
        .mount("/", routes![index])
        .mount("/slope", routes![slope_tiles])
}
