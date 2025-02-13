use crate::tile;
use anyhow::{Context, Result};
use image::ImageFormat;
use image::ImageReader;
use image::Rgba;
use imageproc::drawing::draw_text_mut;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde_json;
use std::env;
use std::io::Cursor;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use tar::Archive;
use time::OffsetDateTime;

const URL: &'static str = "https://sh.dataspace.copernicus.eu/api/v1/process";
const IMAGE_SCRIPT: &'static str = r#"//VERSION=3

function setup() {
  return {
    input: ["B02", "B03", "B04"],
    mosaicking: Mosaicking.ORBIT,
    output: { id:"default", bands: 3}
  }
}

function updateOutputMetadata(scenes, inputMetadata, outputMetadata) {
    outputMetadata.userData = { "scenes":  scenes.orbits }
}

function evaluatePixel(samples) {
  return [ 2.5 * samples[0].B04, 2.5 * samples[0].B03, 2.5 * samples[0].B02 ]
}"#;

pub struct Sentinel {
    cache: PathBuf,
    download_lock: tokio::sync::Mutex<u8>,
}

impl Sentinel {
    pub fn new(cache: PathBuf) -> Self {
        Sentinel {
            cache,
            download_lock: tokio::sync::Mutex::new(0),
        }
    }

    // pub async fn get(&self, point: tile::GeoPoint) -> Result<PathBuf> {
    //     let id = srtm_id(point);
    //     self.get_tile(&id).await
    // }

    pub async fn get(&self, zoom: u8, x: u32, y: u32) -> Result<(String, Vec<u8>)> {
        let nw = tile::square_to_meters(&tile::tile_to_square(zoom, x as f64, y as f64));
        let se =
            tile::square_to_meters(&tile::tile_to_square(zoom, x as f64 + 1.0, y as f64 + 1.0));
        let now = OffsetDateTime::now_utc();
        let before = now - Duration::from_secs(3600 * 24 * 30);
        let request = format_request(nw.x, se.y, se.x, nw.y, before, now, 15.0);
        let (meta, image) = self.download(request).await?;
        let date = self.extract_date(&meta)?;
        let new_image = add_text(&image, &date)?;
        Ok((date, new_image))
    }

    async fn download(&self, request: String) -> Result<(String, Vec<u8>)> {
        println!("request {}", request);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;
        let form = reqwest::multipart::Form::new()
            .text("request", request)
            .text("evalscript", IMAGE_SCRIPT);
        let token = env::var("FLYTILE_SENTINEL_TOKEN")?;
        let to_send = client
            .post(URL)
            .header(AUTHORIZATION, format!("Bearer {}", token))
            .header(ACCEPT, "application/tar")
            .multipart(form)
            .build()?;

        println!("head {:?}", to_send.headers());
        println!("body {:?}", to_send.body());
        let response = client.execute(to_send).await?;
        println!();
        println!("status {:?}", response.status());
        println!("head {:?}", response.headers());
        println!("url {:?}", response.url());
        response.error_for_status_ref()?;

        let content = Cursor::new(response.bytes().await?);
        let mut archive = Archive::new(content);
        let entries = archive.entries()?;
        let mut image = Vec::new();
        let mut meta = String::new();
        for maybe_entry in entries {
            let mut entry = maybe_entry?;
            let path = entry.path()?;
            if path.to_string_lossy() == "default.png" {
                println!("png bytes {}", entry.size());
                entry.read_to_end(&mut image)?;
            } else if path.to_string_lossy() == "userdata.json" {
                println!("json bytes {}", entry.size());
                entry.read_to_string(&mut meta)?;
            }
        }
        // let image = archive.unpack("default.png")?;
        // let meta = archive.unpack("userdata.json");
        // let mut file = fs::File::create(&output)?;
        // io::copy(&mut content, &mut file)?;
        // return Ok(output.to_path_buf());
        // Ok((meta, image))
        Ok((meta, image))
    }

    fn extract_date(&self, meta: &str) -> Result<String> {
        let json: serde_json::Value = serde_json::from_str(meta)?;
        let date = json["scenes"][0]["dateFrom"].as_str().context("oops")?;
        Ok(date[..10].to_string())
    }
}

fn add_text(image: &[u8], text: &str) -> Result<Vec<u8>> {
    let mut tmp = ImageReader::new(Cursor::new(image))
        .with_guessed_format()?
        .decode()?;
    let red = Rgba([255u8, 0u8, 0u8, 127u8]);
    let scale = ab_glyph::PxScale { x: 10.0, y: 10.0 };
    let font = ab_glyph::FontRef::try_from_slice(include_bytes!("DejaVuSans.ttf"))?;
    draw_text_mut(&mut tmp, red, 5, 5, scale, &font, text);
    let mut cursor = Cursor::new(Vec::new());
    tmp.write_to(&mut cursor, ImageFormat::Png)?;
    Ok(cursor.into_inner())
}

fn format_request(
    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,
    start_time: OffsetDateTime,
    end_time: OffsetDateTime,
    max_cloud_coverage: f64,
) -> String {
    let formatter =
        time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]Z").unwrap();
    return format!(
        r#"{{
    "input": {{
        "bounds": {{
           "properties": {{
                "crs": "http://www.opengis.net/def/crs/EPSG/0/3857"
            }},
             "bbox": [
                {min_x},
                {min_y},
                {max_x},
                {max_y}
            ]
        }},
        "data": [
            {{
                "type": "sentinel-2-l1c",
                "dataFilter": {{
                    "timeRange": {{
                        "from": "{start_time}",
                        "to": "{end_time}"
                    }},
                    "maxCloudCoverage": {max_cloud_coverage}
                }}
            }}
        ]
    }},
    "output": {{
        "width": 256,
        "height": 256,
        "responses": [
            {{
                "identifier": "default",
                "format": {{
                    "type": "image/png"
                }}
            }},
            {{
                "identifier": "userdata",
                "format": {{
                    "type": "application/json"
                }}
            }}
        ]
    }}
}}"#,
        start_time = start_time.format(&formatter).unwrap(),
        end_time = end_time.format(&formatter).unwrap()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use time::macros::datetime;
    use tokio::runtime::Runtime;

    #[test]
    fn test_format() {
        println!(
            "{}",
            format_request(
                1.0,
                2.0,
                3.0,
                4.0,
                datetime!(2025-01-01 0:00 UTC),
                datetime!(2025-02-08 0:00 UTC),
                22.3
            )
        );
        assert!(false);
    }
    #[test]
    fn test_get() {
        let runtime = Runtime::new().unwrap();
        let sentinel = Sentinel::new(Path::new("/tmp").into());
        let (meta, image) = runtime.block_on(sentinel.get(12, 669, 1396)).unwrap();
        println!("meta {}", meta);
        println!("image bytes {}", image.len());
        fs::write("test.png", image).unwrap();
        assert!(false);
    }
}
