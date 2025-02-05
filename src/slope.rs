use anyhow::Result;
use std::path;
use std::process;

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

pub fn angle_shade(input: path::PathBuf) -> Result<path::PathBuf> {
    let mut outname = input.file_stem().unwrap().to_os_string();
    outname.push("_angle_shade.png");
    let output = path::Path::new(input.parent().unwrap()).join(outname);
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
    return Ok(output);
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
        let output =
            angle_shade(path::Path::new("/tmp/srtm_13_03_slope.tif").to_path_buf()).unwrap();
        assert_eq!(
            output.to_str().unwrap(),
            "/tmp/srtm_13_03_slope_angle_shade.tif"
        );
        assert!(output.exists());
    }
}
