//! OAuth 2.0 token exchange and refresh.

pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: u64,
}

pub async fn exchange_code(_code: &str) -> anyhow::Result<TokenResponse> {
    todo!()
}

pub async fn refresh_token(_refresh_token: &str) -> anyhow::Result<TokenResponse> {
    todo!()
}
