use crate::tile;
use anyhow::Result;
use std::fs;
use std::path;
use std::process;

pub struct Pipeline {
    cache: path::PathBuf,
    process_lock: tokio::sync::Mutex<u8>,
}

impl Pipeline {
    pub fn new(cache: path::PathBuf) -> Self {
        Pipeline {
            cache,
            process_lock: tokio::sync::Mutex::new(0),
        }
    }

    pub async fn get(
        &self,
        elevation: path::PathBuf,
        zoom: u8,
        x: u32,
        y: u32,
    ) -> Result<path::PathBuf> {
        let output_dir = self.cache.join(format!("{}", zoom)).join(format!("{}", x));
        let output = output_dir.join(format!("{}.png", y));
        let _lock = self.process_lock.lock().await;
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }
        if !output.exists() {
            let elevation_tile = tile::single_tile(elevation, zoom, x as f64, y as f64).unwrap();
            println!("elevation tile {:?}", elevation_tile);
            let slope = slope(elevation_tile)?;
            println!("slope {:?}", slope);
            angle_shade(&slope, &output)?;
            println!("shade {:?}", output);
        }
        if output.exists() {
            return Ok(output);
        }
        return Err(anyhow!("could not process slope data"));
    }
}

pub fn slope(input: path::PathBuf) -> Result<path::PathBuf> {
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push("_slope.tif");
    let output = path::Path::new(input.parent().unwrap()).join(outname);
    let result = process::Command::new("gdaldem")
        .arg("slope")
        .arg("-s")
        // .arg("111120")
        .arg("1.0")
        .arg(&input)
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8_lossy(&result.stderr)));
    }
    return Ok(output);
}

pub fn angle_shade(input: &path::PathBuf, output: &path::PathBuf) -> Result<()> {
    let result = process::Command::new("gdaldem")
        .arg("color-relief")
        .arg("-alpha")
        .arg("-nearest_color_entry")
        .arg(&input)
        .arg("color.txt")
        .arg(&output)
        .output()?;
    if !result.status.success() {
        return Err(anyhow!("{:?}", String::from_utf8_lossy(&result.stderr)));
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slope() {
        let output = slope(path::Path::new("/tmp/srtm_13_03.tif").to_path_buf()).unwrap();
        assert_eq!(output.to_str().unwrap(), "/tmp/srtm_13_03_slope.tif");
        assert!(output.exists());
    }

    #[test]
    fn test_angle_shade() {
        let input = path::Path::new("/tmp/srtm_13_03_slope.tif");
        let output = path::Path::new("/tmp/srtm_13_03_slope_shade.tif");
        angle_shade(&input.to_path_buf(), &output.to_path_buf()).unwrap();
        assert_eq!(
            output.to_str().unwrap(),
            "/tmp/srtm_13_03_slope_angle_shade.tif"
        );
        assert!(output.exists());
    }
}
