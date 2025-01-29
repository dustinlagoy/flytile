use std::path;
use std::process;

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
}
