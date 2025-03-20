use crate::cache;
use crate::processing::{ProcessingError, ProcessingResult};
use crate::tile;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::mpsc;
use tempfile::NamedTempFile;

pub struct Pipeline {
    cache_dir: PathBuf,
    cache_tx: mpsc::Sender<(
        cache::Request,
        Box<dyn FnOnce() -> cache::CacheResult + Send>,
    )>,
    process_lock: tokio::sync::Mutex<u8>,
}

impl Pipeline {
    pub fn new(cache_dir: PathBuf) -> Self {
        let cache = cache::Cache::from_existing_directory(
            cache_dir.clone(),
            10_000_000_000,
            100_000_000,
            86400 * 365,
        )
        .unwrap();
        let cache_tx = cache::run_cache(cache);
        Pipeline {
            cache_dir,
            cache_tx,
            process_lock: tokio::sync::Mutex::new(0),
        }
    }

    pub async fn get(&self, elevations: Vec<PathBuf>, zoom: u8, x: u32, y: u32) -> Result<PathBuf> {
        let key = PathBuf::new()
            .join(format!("{}", zoom))
            .join(format!("{}", x))
            .join(format!("{}.png", y));
        let output = self.cache_dir.join(&key);
        let generator = move || process(output, elevations, zoom, x, y);
        let (tx, rx) = mpsc::channel();
        self.cache_tx
            .send((cache::Request { key, send_back: tx }, Box::new(generator)))
            .unwrap();
        let result = rx.recv()??;
        return Ok(result);
    }
}

fn process(
    output: PathBuf,
    elevations: Vec<PathBuf>,
    zoom: u8,
    x: u32,
    y: u32,
) -> cache::CacheResult {
    let parent = output.parent().expect("output should have parent dir");
    if !parent.exists() {
        fs::create_dir_all(parent)?;
    }
    log::info!("make shaded slope tile {} {} {}", zoom, x, y);
    let vrt_file = NamedTempFile::new()?;
    let vrt_path = vrt_file.path().to_path_buf();
    make_vrt(&elevations, &vrt_path)?;
    let elevation_tile = tile::single_tile(vrt_path, zoom, x as f64, y as f64).unwrap();
    log::debug!("have elevation tile {:?}", elevation_tile);
    // scale slope by cosine of tile center latitude since this is a conformal projection
    // from John P. Snyder https://doi.org/10.3133/pp1395
    // see below test for maximum error of this approximation (1 percent at zoom level 8)
    let tile_center =
        tile::square_to_geodetic(&tile::tile_to_square(zoom, x as f64 + 0.5, y as f64 + 0.5));
    let slope = slope(elevation_tile, tile_center.latitude.to_radians().cos())?;
    log::debug!("have slope tile {:?}", slope);
    angle_shade(&slope, &output)?;
    log::debug!("have shaded tile {:?}", output);
    if output.exists() {
        log::info!("return generated slope tile {:?}", output);
        return Ok(output);
    }
    return Err(cache::GeneratorError::new("could not process slope data"));
}

pub fn slope(input: PathBuf, scale: f64) -> ProcessingResult<PathBuf> {
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push("_slope.tif");
    let output = Path::new(input.parent().unwrap()).join(outname);
    let result = process::Command::new("gdaldem")
        .arg("slope")
        .arg("-s")
        .arg(format!("{}", scale))
        .arg(&input)
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(ProcessingError::new(&format!(
            "{:?}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    return Ok(output);
}

pub fn angle_shade(input: &PathBuf, output: &PathBuf) -> ProcessingResult<()> {
    let result = process::Command::new("gdaldem")
        .arg("color-relief")
        .arg("-alpha")
        .arg("-nearest_color_entry")
        .arg(&input)
        .arg("color.txt")
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(ProcessingError::new(&format!(
            "{:?}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slope() {
        let output = slope(Path::new("/tmp/srtm_13_03.tif").to_path_buf(), 1.0).unwrap();
        assert_eq!(output.to_str().unwrap(), "/tmp/srtm_13_03_slope.tif");
        assert!(output.exists());
    }

    #[test]
    fn test_angle_shade() {
        let input = Path::new("/tmp/srtm_13_03_slope.tif");
        let output = Path::new("/tmp/srtm_13_03_slope_shade.tif");
        angle_shade(&input.to_path_buf(), &output.to_path_buf()).unwrap();
        assert_eq!(
            output.to_str().unwrap(),
            "/tmp/srtm_13_03_slope_angle_shade.tif"
        );
        assert!(output.exists());
    }

    #[test]
    fn test_cosine_approximation() {
        // We approximate corrections to slope numbers with the center latitude
        // of the tile. At zoom level 8 this gives an error of about one percent
        // at 55 degrees latitude, which is 0.4 degrees of slope error at a 40
        // degree slope angle.
        let point = tile::square_to_tile(
            8,
            &tile::geodetic_to_square(&tile::GeoPoint {
                longitude: -130.0,
                latitude: 55.0,
            }),
        );
        let top = tile::square_to_geodetic(&tile::tile_to_square(8, point.x, point.y.floor()));
        let bottom = tile::square_to_geodetic(&tile::tile_to_square(8, point.x, point.y.ceil()));
        let error = 1.0 - top.latitude.to_radians().cos() / bottom.latitude.to_radians().cos();
        let error_from_center = error * 0.5;
        let error_at_forty_degrees = 40.0 * error_from_center;
        assert!(
            error_from_center.abs() < 0.01,
            "fractional error across tile {}",
            error_from_center
        );
        assert!(
            error_at_forty_degrees.abs() < 0.4,
            "slope error at 40 degrees {}",
            error_at_forty_degrees
        );
    }
}

fn make_vrt(paths: &[PathBuf], output: &PathBuf) -> ProcessingResult<()> {
    let result = process::Command::new("gdalbuildvrt")
        .arg(&output)
        .args(paths)
        .output()?;
    if !result.status.success() {
        return Err(ProcessingError::new(&format!(
            "{:?}",
            String::from_utf8_lossy(&result.stderr)
        )));
    }
    return Ok(());
}
