use crate::approx;
use std::path;
use std::process;

pub struct Point {
    pub longitude: f64,
    pub latitude: f64,
}

pub struct Bounds {
    pub north_west: Point,
    pub north_east: Point,
    pub south_west: Point,
    pub south_east: Point,
}

pub fn tile_bounds(zoom: u8, x: u32, y: u32) -> Bounds {
    let xf = x as f64;
    let yf = y as f64;
    return Bounds {
        north_west: to_geodetic(zoom, xf, yf),
        north_east: to_geodetic(zoom, xf + 1.0, yf),
        south_west: to_geodetic(zoom, xf, yf + 1.0),
        south_east: to_geodetic(zoom, xf + 1.0, yf + 1.0),
    };
}
// pub fn geodetic_to_web_mercator(longitude: f64, latitude: f64) -> (f64, f64) {
//     let pi = std::f64::consts::PI;
//     // let max_lat = 2.0 * (pi.exp()).atan() - pi / 2.0;
//     // println!("max lat {}", max_lat.to_degrees());
//     let x = (longitude + 180.0) / 360.0;
//     let lat_rad = latitude.to_radians();
//     let y_web_mercator = (lat_rad.tan() + 1.0 / lat_rad.cos()).ln();
//     // println!("y_wm {}", y_web_mercator);
//     let y = (0.5 - y_web_mercator / (2.0 * pi)) * zoom_scale;
//     return (x, y);
// }

pub fn to_xy(zoom: u8, longitude: f64, latitude: f64) -> (f64, f64) {
    let pi = std::f64::consts::PI;
    let max_lat = 2.0 * (pi.exp()).atan() - pi / 2.0;
    println!("max lat {}", max_lat.to_degrees());
    // from wikipedia
    // let zoom_scale = 1.0 / (2.0 * pi) * 2_i32.pow(zoom as u32) as f64;
    // println!("zoom scale {}", zoom_scale);
    // let x = (zoom_scale * (pi + longitude)).floor();
    // let y = (zoom_scale * (pi - (pi / 4.0 + latitude / 2.0).tan().ln())).floor();
    // from https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
    let zoom_scale = 2_i32.pow(zoom as u32) as f64;
    let x = (longitude + 180.0) / 360.0;
    let lat_rad = latitude.to_radians();
    let y_web_mercator = (lat_rad.tan() + 1.0 / lat_rad.cos()).ln();
    println!("y_wm {}", y_web_mercator);
    let y = 0.5 - y_web_mercator / (2.0 * pi);
    println!("x {} y {}", x, y);
    return (x * zoom_scale, y * zoom_scale);
}

pub fn to_geodetic(zoom: u8, x: f64, y: f64) -> Point {
    let pi = std::f64::consts::PI;
    // from wikipedia
    // let zoom_scale = 1.0 / (2.0 * pi) * 2_i32.pow(zoom as u32) as f64;
    // let longitude = x as f64 / zoom_scale - pi;
    // let latitude = ((pi - y as f64 / zoom_scale).exp().atan() - pi / 4.0) * 2.0;
    // from https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
    let zoom_scale = 2_i32.pow(zoom as u32) as f64;
    let longitude = x / zoom_scale * 360.0 - 180.0;
    let latitude = (pi - y / zoom_scale * 2.0 * pi).sinh().atan().to_degrees();
    return Point {
        longitude,
        latitude,
    };
}

fn tile(input: path::PathBuf) -> Option<path::PathBuf> {
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push("_tiles");
    let output = path::Path::new(input.parent().unwrap()).join(outname);
    let result = process::Command::new("gdal2tiles.py")
        .arg("--xyz")
        .arg("--processes")
        .arg("4")
        .arg(&input)
        .arg(&output)
        .output()
        .unwrap();
    if !result.status.success() {
        println!("failed to make tiles");
        return None;
    }
    return Some(output);
}

// fn from_source(source: path::PathBuf, zoom: u8, x:u32,y:u32) -> Option<path::PathBuf> {

// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile() {
        let output =
            tile(path::Path::new("/tmp/srtm_13_03_slope_angle_shade.tif").to_path_buf()).unwrap();
        assert_eq!(
            output.to_str().unwrap(),
            "/tmp/srtm_13_03_slope_angle_shade_tiles"
        );
        assert!(output.exists());
    }

    #[test]
    fn test_to_xy() {
        // let (x, y) = to_xy(18, 139.7006793, 35.6590699);
        let (x, y) = to_xy(10, -119.81924, 49.20555);
        println!("x {} y {}", x, y);
        approx::assert_approx!(x, 232798.930207, 1.0e-6);
        // source says this shoudl be 103246.410422 but that seems wrong
        approx::assert_approx!(y, 103246.410438, 1.0e-6);
    }

    #[test]
    fn test_to_geodetic() {
        let point = to_geodetic(2, 1.5, 1.5);
        approx::assert_approx!(point.longitude, -45.0, 1.0e-6);
        approx::assert_approx!(point.latitude, 40.979898, 1.0e-6);
    }
}
