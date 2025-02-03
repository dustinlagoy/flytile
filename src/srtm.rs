use crate::tile;
use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::time;
extern crate reqwest;

// const URL: &'static str = "https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_{i_lon:02d}_{i_lat:02d}.zip";

// def download(i_lat, i_lon):
//     url = f"https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/{_basename(i_lat, i_lon)}.zip"
//     print("downloading", url)
//     response = urllib3.request("GET", url)
//     assert response.status == 200
//     os.makedirs(TMP, exist_ok=True)
//     with zipfile.ZipFile(io.BytesIO(response.data)) as data:
//         for item in data.infolist():
//             with data.open(item) as memory_file:
//                 out_name = f"{TMP}/{item.filename}"
//                 # print("write", out_name)
//                 with open(out_name, "wb") as out_file:
//                     out_file.write(memory_file.read())

pub fn srtm_tile(point: tile::GeoPoint) -> (i32, i32) {
    // lat = 60 - 5 * (index - 1)
    // lon = -180 + 5 * (index - 1)
    let i_lon: i32 = ((180.0 + point.longitude) / 5.0) as i32 + 1;
    let i_lat: i32 = ((60.0 - point.latitude) / 5.0) as i32 + 1;
    return (i_lon, i_lat);
}

pub fn retrieve_all(bounds: tile::Bounds) -> Option<PathBuf> {
    let (nw_lon, nw_lat) = srtm_tile(bounds.north_west);
    let (ne_lon, ne_lat) = srtm_tile(bounds.north_east);
    let (sw_lon, sw_lat) = srtm_tile(bounds.south_west);
    let (se_lon, se_lat) = srtm_tile(bounds.south_east);
    let min_lon = nw_lon.min(sw_lon);
    let min_lat = sw_lat.min(se_lat);
    let max_lon = ne_lon.max(se_lon);
    let max_lat = nw_lat.max(ne_lat);
    let mut files = vec![];
    for i in min_lon..max_lon + 1 {
        for j in min_lat..max_lat + 1 {
            files.push(download(i, j)?);
        }
    }
    return make_vrt(&files);
}

pub fn get(point: tile::GeoPoint) -> Option<PathBuf> {
    let (i_lon, i_lat) = srtm_tile(point);
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    let output = Path::new(&cache)
        .join("srtm")
        .join(format!("srtm_{:02}_{:02}.tif", i_lon, i_lat));
    if !output.exists() {
        extract(download(i_lon, i_lat)?);
    }
    if output.exists() {
        return Some(output);
    }
    None
}

pub fn download(i_lon: i32, i_lat: i32) -> Option<PathBuf> {
    let url = format!(
        "https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_{i_lon:02}_{i_lat:02}.zip",
        i_lon=i_lon,
        i_lat=i_lat
    );
    let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
    let output = Path::new(&cache)
        .join("srtm")
        .join(format!("{}_{}.zip", i_lon, i_lat));
    let mut file = fs::File::create(&output).ok()?;
    println!("downloading {}", url);
    let client = reqwest::blocking::Client::builder()
        .timeout(time::Duration::from_secs(300))
        .build()
        .ok()?;
    let response = client.get(url).send().ok()?;
    response.error_for_status_ref().ok()?;
    let mut content = Cursor::new(response.bytes().ok()?);
    io::copy(&mut content, &mut file).ok()?;
    return Some(output.to_path_buf());
}

fn extract(path: PathBuf) {
    let file = fs::File::open(&path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let parent = path.parent().unwrap();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = Path::new(&parent).join(file.enclosed_name().unwrap());
        println!("extracting {:?}", outpath);
        let mut outfile = fs::File::create(&outpath).unwrap();
        io::copy(&mut file, &mut outfile).unwrap();
    }
}

fn make_vrt(paths: &Vec<PathBuf>) -> Option<PathBuf> {
    let output = env::temp_dir().join("tmp.vrt");
    let result = process::Command::new("gdalbuildvrt")
        .arg(&output)
        .args(paths)
        .output()
        .unwrap();
    if !result.status.success() {
        println!("failed to make slope");
        return None;
    }
    return Some(output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download() {
        let (i_lat, i_lon) = srtm_tile(tile::GeoPoint {
            longitude: -120.0,
            latitude: 50.0,
        });
        let path = download(i_lat, i_lon).unwrap();
        assert_eq!(path.to_str().unwrap(), "/tmp/13_3.zip");
        extract(path);
        assert!(Path::new("/tmp/srtm_13_03.hdr").exists());
    }
}
