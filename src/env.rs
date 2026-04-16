use std::sync::LazyLock;

use openai_client::OpenAIAuth;

#[must_use]
#[inline]
pub fn openai_auth_from_string(string: String) -> Option<OpenAIAuth> {
    if string.contains('|') {
        return string.split_once('|').map(|(k, v)| OpenAIAuth::ApiKey {
            key: k.to_string(),
            value: v.to_string(),
        });
    }
    Some(OpenAIAuth::BearerToken(string))
}

pub struct EnvVars {
    pub api_key: Option<OpenAIAuth>,
    pub v1_endpoint: String,
}

pub static ENV_VARS: LazyLock<EnvVars> = LazyLock::new(|| {
    let api_key: Option<OpenAIAuth> = dotenvy::var("API_KEY")
        .ok()
        .and_then(openai_auth_from_string);
    let v1_endpoint = dotenvy::var("V1_ENDPOINT").expect("Please provied a v1 endpoint in .env");
    EnvVars {
        api_key,
        v1_endpoint,
    }
});
