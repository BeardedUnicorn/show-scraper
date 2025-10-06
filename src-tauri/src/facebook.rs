use thiserror::Error;

use crate::config::AppConfig;

#[derive(Debug, Error)]
pub enum FacebookError {
    #[error("missing facebook token")]
    MissingToken,
    #[error("no facebook group configured")]
    MissingGroup,
    #[error("http error: {0}")]
    Http(String),
    #[error("facebook api error: {0}")]
    Api(String),
}

pub struct FbPoster {
    token: String,
    group_id: String,
}

impl FbPoster {
    pub fn from_config(config: &AppConfig) -> Result<Self, FacebookError> {
        let token = config
            .facebook_access_token
            .as_ref()
            .ok_or(FacebookError::MissingToken)?
            .trim()
            .to_string();
        if token.is_empty() {
            return Err(FacebookError::MissingToken);
        }

        let group_id = config
            .facebook_group_id
            .as_ref()
            .ok_or(FacebookError::MissingGroup)?
            .trim()
            .to_string();
        if group_id.is_empty() {
            return Err(FacebookError::MissingGroup);
        }

        Ok(Self { token, group_id })
    }

    pub async fn post(&self, message: &str) -> Result<String, FacebookError> {
        let client = reqwest::Client::new();
        let url = format!("https://graph.facebook.com/v19.0/{}/feed", self.group_id);

        let response = client
            .post(url)
            .form(&[("message", message), ("access_token", &self.token)])
            .send()
            .await
            .map_err(|err| FacebookError::Http(err.to_string()))?;

        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|err| FacebookError::Http(err.to_string()))?;

        if !status.is_success() {
            return Err(FacebookError::Api(body.to_string()));
        }

        let id = body
            .get("id")
            .and_then(|val| val.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown_post_id".to_string());

        Ok(id)
    }
}
