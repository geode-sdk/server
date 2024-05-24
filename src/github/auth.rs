pub struct GitHubAppData {
    token: Option<String>,
    private_key: String,
}

impl GitHubAppData {
    pub fn new(private_key: &str) -> GitHubAppData {
        GitHubAppData {
            token: None,
            private_key: String::from(private_key),
        }
    }

    pub async fn auth(&self) -> Result<String, String> {
        Ok(String::from(""))
    }

    pub fn is_auth(&self) -> bool {
        self.token.is_some()
    }

    pub fn get_token(&self) -> Option<String> {
        Some(String::from(""))
    }
}
