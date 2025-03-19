use crate::cache;
use crate::tile;
use anyhow::Result;
use std::env;
use std::fs;
use std::io;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time;
use tokio;
extern crate reqwest;

// const URL: &'static str = "https://srtm.csi.cgiar.org/wp-content/uploads/files/srtm_5x5/TIFF/srtm_{i_lon:02d}_{i_lat:02d}.zip";

fn redirect_with_auth_and_cookies(
    url: &str,
) -> std::result::Result<reqwest::blocking::Response, cache::GeneratorError> {
    log::debug!("retrieving {}", url);
    let password = env::var("FLYTILE_SRTM_PASSWORD")?;
    let mut jar = Some(reqwest::cookie::Jar::default());
    let mut new_url = url.to_string();
    for _i in 0..10 {
        let client = reqwest::blocking::Client::builder()
            .cookie_provider(std::sync::Arc::new(jar.take().unwrap()))
            .redirect(reqwest::redirect::Policy::none())
            .timeout(time::Duration::from_secs(180))
            .build()?;
        let response = client
            .get(&new_url)
            .basic_auth("dustin.lagoy", Some(&password))
            .send()?;
        log::debug!("response status {:?}", response.status());
        log::debug!("response headers {:?}", response.headers());
        log::debug!("response url {:?}", response.url());
        if response.status() == 200 {
            return Ok(response);
        }
        new_url = response.headers()[reqwest::header::LOCATION]
            .to_str()?
            .to_string();
        let tmp_jar = reqwest::cookie::Jar::default();
        for cookie in response.headers().get_all(reqwest::header::SET_COOKIE) {
            let cookie_url = response.url();
            log::debug!("add cookie {} for {}", cookie.to_str()?, &cookie_url);
            tmp_jar.add_cookie_str(cookie.to_str()?, &cookie_url);
        }
        jar = Some(tmp_jar);
    }
    return Err(cache::GeneratorError::new("too many redirects"));
}

pub struct SRTM {
    cache_dir: PathBuf,
    cache_tx: mpsc::Sender<(
        cache::Request,
        Box<dyn FnOnce() -> cache::CacheResult + Send>,
    )>,
    download_lock: tokio::sync::Mutex<u8>,
}

impl SRTM {
    pub fn new(cache_dir: PathBuf) -> Self {
        let cache = cache::Cache::from_existing_directory(
            cache_dir.clone(),
            10_000_000_000,
            100_000_000,
            86400 * 365,
        )
        .unwrap();
        let tx = cache::run_cache(cache);
        SRTM {
            cache_dir,
            cache_tx: tx,
            download_lock: tokio::sync::Mutex::new(0),
        }
    }

    pub async fn get(&self, point: tile::GeoPoint) -> Result<PathBuf> {
        let id = srtm_id(&point);
        log::debug!("get srtm {} for point {:?}", id, point);
        self.get_tile(&id).await
    }

    pub async fn get_tile(&self, id: &str) -> Result<PathBuf> {
        let out_dir = self.cache_dir.clone();
        let tmp: String = id.to_string();
        let generator = move || download_tile(out_dir, &tmp);
        let (tx, rx) = mpsc::channel();
        self.cache_tx
            .send((
                cache::Request {
                    key: id.into(),
                    send_back: tx,
                },
                Box::new(generator),
            ))
            .unwrap();
        let result = rx.recv()??;
        return Ok(result);
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
        log::debug!("lon bounds {} {}", min_lon, max_lon);
        log::debug!("lat bounds {} {}", min_lat, max_lat);
        for i in min_lon..max_lon {
            for j in min_lat..max_lat {
                let point = tile::GeoPoint {
                    longitude: i as f64,
                    latitude: j as f64,
                };
                files.push(self.get(point).await?);
            }
        }
        return Ok(files);
    }
}

fn download_tile(output_directory: PathBuf, id: &str) -> cache::CacheResult {
    let url = format!(
        "https://e4ftl01.cr.usgs.gov/MEASURES/SRTMGL1.003/2000.02.11/{}.SRTMGL1.hgt.zip",
        id
    );
    let zipfile = Path::new("/tmp").join(format!("{}.zip", id));
    log::info!("downloading srtm image {}", id);
    let response = redirect_with_auth_and_cookies(&url)?;
    response.error_for_status_ref()?;
    let mut content = Cursor::new(response.bytes()?);
    let mut file = fs::File::create(&zipfile)?;
    io::copy(&mut content, &mut file)?;
    let mut outputs = extract(output_directory, zipfile);
    return outputs
        .pop()
        .ok_or(cache::GeneratorError::new("no outputs"));
}

fn extract(output_directory: PathBuf, path: PathBuf) -> Vec<PathBuf> {
    let file = fs::File::open(&path).unwrap();
    let mut archive = zip::ZipArchive::new(file).unwrap();
    let mut outputs = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = Path::new(&output_directory).join(file.enclosed_name().unwrap());
        log::debug!("extracting {:?}", outpath);
        let mut outfile = fs::File::create(&outpath).unwrap();
        io::copy(&mut file, &mut outfile).unwrap();
        outputs.push(outpath);
    }
    outputs
}

pub fn srtm_id(point: &tile::GeoPoint) -> String {
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
    output.push_str(".hgt");
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id() {
        let id = srtm_id(&tile::GeoPoint {
            latitude: 44.6474,
            longitude: -121.5847,
        });
        assert_eq!(id, "N44W122");
        let id = srtm_id(&tile::GeoPoint {
            latitude: -4.11909,
            longitude: 22.55864,
        });
        assert_eq!(id, "S05E022");
    }

    #[test]
    fn test_download() {
        let id = srtm_id(&tile::GeoPoint {
            longitude: -120.0,
            latitude: 50.0,
        });
        let result = download_tile(PathBuf::new().join("/tmp"), &id).unwrap();
        assert_eq!(result.to_string_lossy(), "/tmp/srtm_13_03.hdr");
        assert!(Path::new("/tmp/srtm_13_03.hdr").exists());
    }
}
