use crate::tile;
use anyhow::Result;
use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::time;
use tokio;
extern crate reqwest;

// const URL: &'static str = "https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_{i_lon:02d}_{i_lat:02d}.zip";

async fn redirect_with_auth_and_cookies(url: &str) -> Result<reqwest::Response> {
    println!("downloading {}", url);
    let password = env::var("FLYTILE_SRTM_PASSWORD")?;
    let mut jar = Some(reqwest::cookie::Jar::default());
    let mut new_url = url.to_string();
    for i in 0..10 {
        let client = reqwest::Client::builder()
            .cookie_provider(std::sync::Arc::new(jar.take().unwrap()))
            .redirect(reqwest::redirect::Policy::none())
            .timeout(time::Duration::from_secs(180))
            .build()?;
        let response = client
            .get(&new_url)
            .basic_auth("dustin.lagoy", Some(&password))
            .send()
            .await?;
        println!();
        println!("{} status {:?}", i, response.status());
        println!("{} head {:?}", i, response.headers());
        println!("{} url {:?}", i, response.url());
        if response.status() == 200 {
            return Ok(response);
        }
        new_url = response.headers()[reqwest::header::LOCATION]
            .to_str()?
            .to_string();
        let tmp_jar = reqwest::cookie::Jar::default();
        for cookie in response.headers().get_all(reqwest::header::SET_COOKIE) {
            let cookie_url = response.url();
            println!("add cookie {} for {}", cookie.to_str()?, &cookie_url);
            tmp_jar.add_cookie_str(cookie.to_str()?, &cookie_url);
        }
        jar = Some(tmp_jar);
    }
    return Err(anyhow!("too many redirects"));
}

pub struct SRTM {
    cache: PathBuf,
    download_lock: tokio::sync::Mutex<u8>,
}

impl SRTM {
    pub fn new(cache: PathBuf) -> Self {
        SRTM {
            cache,
            download_lock: tokio::sync::Mutex::new(0),
        }
    }

    pub async fn get(&self, point: tile::GeoPoint) -> Result<PathBuf> {
        let id = srtm_id(point);
        self.get_tile(&id).await
    }

    pub async fn get_tile(&self, id: &str) -> Result<PathBuf> {
        let cache = env::var("FLYTILE_CACHE_DIR").unwrap_or("/tmp".into());
        let output_dir = Path::new(&cache).join("srtm");
        let output = output_dir.join(format!("{}.hgt", id));
        let _lock = self.download_lock.lock().await;
        if !output_dir.exists() {
            fs::create_dir_all(output_dir)?;
        }
        println!("trying srtm {:?}", output);
        if !output.exists() {
            self.extract(self.download(id).await?);
        }
        if output.exists() {
            return Ok(output);
        }
        return Err(anyhow!("could not save srtm data"));
    }

    pub async fn get_all(&self, bounds: tile::Bounds) -> Result<Vec<PathBuf>> {
        let min_lon = bounds
            .north_west
            .longitude
            .min(bounds.south_west.longitude)
            .floor() as i32;
        let min_lat = bounds
            .south_west
            .latitude
            .min(bounds.south_east.latitude)
            .floor() as i32;
        let max_lon = bounds
            .north_east
            .longitude
            .max(bounds.south_east.longitude)
            .ceil() as i32;
        let max_lat = bounds
            .north_west
            .latitude
            .max(bounds.north_east.latitude)
            .ceil() as i32;
        let mut files = vec![];
        // println!("bounds {:?}", bounds);
        // println!("lon {} {}", min_lon, max_lon);
        // println!("lat {} {}", min_lat, max_lat);
        for i in min_lon..max_lon {
            for j in min_lat..max_lat {
                let point = tile::GeoPoint {
                    longitude: i as f64,
                    latitude: j as f64,
                };
                // println!("get file {} {} {}", i, j, id);
                files.push(self.get(point).await?);
            }
        }
        return Ok(files);
    }

    pub async fn download(&self, id: &str) -> Result<PathBuf> {
        let url = format!(
            "https://e4ftl01.cr.usgs.gov/MEASURES/SRTMGL1.003/2000.02.11/{}.SRTMGL1.hgt.zip",
            id
        );
        let output = Path::new(&self.cache).join(format!("{}.zip", id));
        println!("downloading {}", url);
        let response = redirect_with_auth_and_cookies(&url).await?;
        response.error_for_status_ref()?;
        let mut content = Cursor::new(response.bytes().await?);
        let mut file = fs::File::create(&output)?;
        io::copy(&mut content, &mut file)?;
        return Ok(output.to_path_buf());
    }

    fn extract(&self, path: PathBuf) {
        let file = fs::File::open(&path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            let outpath = Path::new(&self.cache).join(file.enclosed_name().unwrap());
            println!("extracting {:?}", outpath);
            let mut outfile = fs::File::create(&outpath).unwrap();
            io::copy(&mut file, &mut outfile).unwrap();
        }
    }
}

pub fn srtm_id(point: tile::GeoPoint) -> String {
    let mut output = String::from("");
    if point.latitude >= 0.0 {
        output.push_str(&format!("N{:02}", point.latitude.floor()));
    } else {
        output.push_str(&format!("S{:02}", point.latitude.abs().ceil()));
    }
    if point.longitude >= 0.0 {
        output.push_str(&format!("E{:03}", point.longitude.floor()));
    } else {
        output.push_str(&format!("W{:03}", point.longitude.abs().ceil()));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_id() {
        let id = srtm_id(tile::GeoPoint {
            latitude: 44.6474,
            longitude: -121.5847,
        });
        assert_eq!(id, "N44W122");
        let id = srtm_id(tile::GeoPoint {
            latitude: -4.11909,
            longitude: 22.55864,
        });
        assert_eq!(id, "S05E022");
    }

    #[test]
    fn test_download() {
        let id = srtm_id(tile::GeoPoint {
            longitude: -120.0,
            latitude: 50.0,
        });
        let srtm = SRTM::new(Path::new("/tmp").into());
        let runtime = Runtime::new().unwrap();
        let path = runtime.block_on(srtm.download(&id)).unwrap();
        assert_eq!(path.to_str().unwrap(), "/tmp/13_3.zip");
        srtm.extract(path);
        assert!(Path::new("/tmp/srtm_13_03.hdr").exists());
    }
}
