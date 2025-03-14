use crate::processing::ProcessingError;
use serde::{de::Error, Deserialize, Deserializer};
use std::env;
use std::time;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct Token {
    access_token: String,
    #[serde(
        rename(deserialize = "expires_in"),
        deserialize_with = "from_expiration"
    )]
    expiration: time::Instant,
}

fn from_expiration<'de, D>(deserializer: D) -> Result<time::Instant, D::Error>
where
    D: Deserializer<'de>,
{
    let duration: u64 = Deserialize::deserialize(deserializer)?;
    Ok(time::Instant::now() + time::Duration::from_secs(duration))
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Token: \"{}...\" Expires in: {}",
            self.access_token[..6].to_string(),
            (self.expiration - time::Instant::now()).as_secs_f64()
        )
    }
}

pub struct Generator {
    url: String,
    token: Option<Token>,
}

impl Generator {
    pub fn new<T: Into<String>>(url: T) -> Self {
        Generator {
            url: url.into(),
            token: None,
        }
    }

    pub fn get(&mut self) -> std::result::Result<String, ProcessingError> {
        // TODO: use message passing for token requests
        if self
            .token
            .as_ref()
            .is_none_or(|x| (x.expiration - time::Instant::now()).as_secs() < 15)
        {
            // regenerate if token is missing or about to expire
            self.token = Some(self.generate()?);
        }
        Ok(self.token.as_ref().unwrap().access_token.clone())
    }

    fn generate(&self) -> std::result::Result<Token, ProcessingError> {
        log::debug!("requesting token from {}", self.url);
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?;
        // TODO: cache these?
        let id = env::var("FLYTILE_SENTINEL_ID")?;
        let secret = env::var("FLYTILE_SENTINEL_SECRET")?;
        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &id),
            ("client_secret", &secret),
        ];
        let to_send = client.post(&self.url).form(&params).build()?;

        log::debug!("send headers {:?}", to_send.headers());
        log::debug!("send body {:?}", to_send.body());
        let response = client.execute(to_send)?;
        log::debug!("response status {:?}", response.status());
        log::debug!("response headers {:?}", response.headers());
        log::debug!("response url {:?}", response.url());
        response.error_for_status_ref()?;
        let token = response.json::<Token>()?;
        log::debug!("received token {}", token);
        Ok(token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_type() {
        let stuff = r#"{"access_token": "hi", "expires_in": 600}"#;
        let token: Token = serde_json::from_str(stuff).unwrap();
        println!("token {:?}", token);
        assert_eq!(token.access_token, "hi");
        assert!((token.expiration - time::Instant::now()).as_secs() >= 599);
        assert!((token.expiration - time::Instant::now()).as_secs() <= 600);
    }
}
