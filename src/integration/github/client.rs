use reqwest::header::{HeaderValue, ACCEPT};
use reqwest::{Client, Method, RequestBuilder, Url};
use url::ParseError;

pub struct GitHubClientBuilder {
    client_id: String,
    client_secret: String,

    req_client: Client,
    base_url: Url,
    with_basic_auth: bool,
}

impl GitHubClientBuilder {
    pub fn new(client_id: String, client_secret: String) -> GitHubClientBuilder {
        let client = Client::builder().user_agent("geode-sdk/index");

        GitHubClientBuilder {
            client_id,
            client_secret,
            // If unwrap() fails here we've got worse things to worry about
            req_client: client.build().unwrap(),
            // If unwrap() fails here the IETF has gone rogue probably
            base_url: Url::parse("https://github.com").unwrap(),
            with_basic_auth: false,
        }
    }

    pub fn get_client_id(&self) -> String {
        self.client_id.clone()
    }

    pub fn get_client_secret(&self) -> String {
        self.client_secret.clone()
    }

    pub fn base_url(mut self, base_url: Url) -> GitHubClientBuilder {
        self.base_url = base_url;
        self
    }

    pub fn with_basic_auth(mut self, with_basic_auth: bool) -> GitHubClientBuilder {
        self.with_basic_auth = with_basic_auth;
        self
    }

    pub fn build(&self, method: Method, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        let final_url = self.base_url.join(endpoint)?;
        let mut request_builder = self
            .req_client
            .request(method, final_url)
            .header(ACCEPT, HeaderValue::from_static("application/json"));

        if self.with_basic_auth {
            request_builder = request_builder.basic_auth(self.client_id, Some(self.client_secret));
        }

        Ok(request_builder)
    }

    pub fn get(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::GET, endpoint)
    }

    pub fn post(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::POST, endpoint)
    }

    pub fn delete(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::DELETE, endpoint)
    }

    pub fn head(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::HEAD, endpoint)
    }

    pub fn options(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::OPTIONS, endpoint)
    }

    pub fn put(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::PUT, endpoint)
    }

    pub fn patch(&self, endpoint: &str) -> Result<RequestBuilder, ParseError> {
        self.build(Method::PATCH, endpoint)
    }
}
