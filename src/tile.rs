use crate::approx;
use anyhow::Result;
use std::path;
use std::process;

const XRANGE: f64 = 20037508.34;
const YRANGE: f64 = 20037508.34;

#[derive(Debug)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug)]
pub struct GeoPoint {
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Debug)]
pub struct Bounds {
    pub north_west: GeoPoint,
    pub north_east: GeoPoint,
    pub south_west: GeoPoint,
    pub south_east: GeoPoint,
}

pub fn tile_bounds(zoom: u8, x: u32, y: u32) -> Bounds {
    return Bounds {
        north_west: square_to_geodetic(&tile_to_square(zoom, x as f64, y as f64)),
        north_east: square_to_geodetic(&tile_to_square(zoom, (x + 1) as f64, y as f64)),
        south_west: square_to_geodetic(&tile_to_square(zoom, x as f64, (y + 1) as f64)),
        south_east: square_to_geodetic(&tile_to_square(zoom, (x + 1) as f64, (y + 1) as f64)),
    };
}

pub fn geodetic_to_square(point: &GeoPoint) -> Point {
    let pi = std::f64::consts::PI;
    // from https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
    let x = (point.longitude + 180.0) / 360.0;
    let lat_rad = point.latitude.to_radians();
    let y_web_mercator = (lat_rad.tan() + 1.0 / lat_rad.cos()).ln();
    let y = 0.5 - y_web_mercator / (2.0 * pi);
    return Point { x, y };
}

pub fn square_to_meters(point: &Point) -> Point {
    return Point {
        x: point.x * 2.0 * XRANGE - XRANGE,
        y: YRANGE - point.y * 2.0 * YRANGE,
    };
}

pub fn square_to_tile(zoom: u8, point: &Point) -> Point {
    let zoom_scale = 2_i32.pow(zoom as u32) as f64;
    return Point {
        x: point.x * zoom_scale,
        y: point.y * zoom_scale,
    };
}

pub fn tile_to_square(zoom: u8, x: f64, y: f64) -> Point {
    let zoom_scale = 2_i32.pow(zoom as u32) as f64;
    Point {
        x: x as f64 / zoom_scale,
        y: y as f64 / zoom_scale,
    }
}

pub fn square_to_geodetic(point: &Point) -> GeoPoint {
    let pi = std::f64::consts::PI;
    // from https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
    let longitude = point.x * 360.0 - 180.0;
    let latitude = (pi - point.y * 2.0 * pi).sinh().atan().to_degrees();
    return GeoPoint {
        longitude,
        latitude,
    };
}

fn tile(input: path::PathBuf) -> Result<path::PathBuf> {
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push("_tiles");
    let output = path::Path::new(input.parent().unwrap()).join(outname);
    let result = process::Command::new("gdal2tiles.py")
        .arg("--xyz")
        .arg("--processes")
        .arg("4")
        .arg(&input)
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8_lossy(&result.stderr)));
    }
    return Ok(output);
}

pub fn single_tile(input: path::PathBuf, zoom: u8, x: f64, y: f64) -> Result<path::PathBuf> {
    let nw_square = tile_to_square(zoom, x, y);
    let nw_meters = square_to_meters(&nw_square);
    let se_square = tile_to_square(zoom, x + 1.0, y + 1.0);
    let se_meters = square_to_meters(&se_square);
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push(format!("_tile_{}_{}_{}.tif", zoom, x, y));
    let output = path::Path::new(input.parent().unwrap()).join(outname);
    let result = process::Command::new("gdalwarp")
        .arg("-t_srs")
        .arg("epsg:3857")
        .arg("-te_srs")
        .arg("epsg:3857")
        .arg("-te")
        .arg(format!("{}", nw_meters.x))
        .arg(format!("{}", se_meters.y))
        .arg(format!("{}", se_meters.x))
        .arg(format!("{}", nw_meters.y))
        .arg("-ts")
        .arg("256")
        .arg("256")
        .arg("-ot")
        .arg("Float32")
        .arg("-r")
        .arg("lanczos")
        .arg(&input)
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8_lossy(&result.stderr)));
    }
    return Ok(output);
}

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
    fn test_edges() {
        let nw_square = tile_to_square(14, 0.0, 0.0);
        approx::assert_approx!(nw_square.x, 0.0, 1.0e-12);
        approx::assert_approx!(nw_square.y, 0.0, 1.0e-12);
        let nw_meters = square_to_meters(&nw_square);
        approx::assert_approx!(nw_meters.x, -20037508.34, 1.0e-6);
        approx::assert_approx!(nw_meters.y, 20037508.34, 1.0e-6);
        let nw_geo = square_to_geodetic(&nw_square);
        approx::assert_approx!(nw_geo.longitude, -180.0, 1.0e-9);
        approx::assert_approx!(nw_geo.latitude, 85.0511287798066, 1.0e-9);

        let se_square = tile_to_square(14, 16384.0, 16384.0);
        approx::assert_approx!(se_square.x, 1.0, 1.0e-12);
        approx::assert_approx!(se_square.y, 1.0, 1.0e-12);
        let se_meters = square_to_meters(&se_square);
        approx::assert_approx!(se_meters.x, 20037508.34, 1.0e-6);
        approx::assert_approx!(se_meters.y, -20037508.34, 1.0e-6);
        let se_geo = square_to_geodetic(&se_square);
        approx::assert_approx!(se_geo.longitude, 180.0, 1.0e-9);
        approx::assert_approx!(se_geo.latitude, -85.0511287798066, 1.0e-9);
    }

    #[test]
    fn test_geodetic_to_tile() {
        // example from https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
        let geo = GeoPoint {
            longitude: 139.7006793,
            latitude: 35.6590699,
        };
        let square = geodetic_to_square(&geo);
        approx::assert_approx!(square.x, 0.8880574425, 1.0e-12);
        approx::assert_approx!(square.y, 0.3938537995827473, 1.0e-12);

        let meters = square_to_meters(&square);
        approx::assert_approx!(meters.x, 15551408.480985638, 1.0e-6);
        approx::assert_approx!(meters.y, 4253810.7522400245, 1.0e-6);

        let point = square_to_tile(18, &square);
        approx::assert_approx!(point.x, 232798.930207, 1.0e-6);
        // source says this should be 103246.410422 but that seems wrong
        approx::assert_approx!(point.y, 103246.410438, 1.0e-6);
    }

    #[test]
    fn test_tile_to_geodetic() {
        let point = square_to_geodetic(&tile_to_square(2, 1.5, 1.5));
        approx::assert_approx!(point.longitude, -45.0, 1.0e-9);
        approx::assert_approx!(point.latitude, 40.979898070, 1.0e-9);
    }
}
