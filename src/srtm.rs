use std::fs;
use std::io;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
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

pub fn tile(latitude: f32, longitude: f32) -> (i32, i32) {
    // lat = 60 - 5 * (index - 1)
    // lon = -180 + 5 * (index - 1)
    let i_lat: i32 = ((60.0 - latitude) / 5.0) as i32 + 1;
    let i_lon: i32 = ((180.0 + longitude) / 5.0) as i32 + 1;
    return (i_lat, i_lon);
}

pub fn download(i_lat: i32, i_lon: i32) -> Option<PathBuf> {
    let url = format!(
        "https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_{i_lon:02}_{i_lat:02}.zip",
        i_lon=i_lon,
        i_lat=i_lat
    );
    let output = Path::new("/tmp").join(format!("{}_{}.zip", i_lon, i_lat));
    let mut file = fs::File::create(&output).ok()?;
    println!("downloading {}", url);
    let response = reqwest::blocking::get(url).ok()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download() {
        let (i_lat, i_lon) = tile(50.0, -120.0);
        let path = download(i_lat, i_lon).unwrap();
        assert_eq!(path.to_str().unwrap(), "/tmp/13_3.zip");
        extract(path);
        assert!(Path::new("/tmp/srtm_13_03.hdr").exists());
    }
}
