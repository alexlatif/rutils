use crate::errors::prelude::*;
use regex::Regex;
use reqwest::{Client, Method, Url};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{error, info};

// pub use reqwest::Method;

#[derive(Default)]
pub struct EndpointBuilder {
    base_url: Option<String>,
    endpoint: Option<String>,
    method: Option<Method>,
    json_body: Option<Value>,
    query_params: Option<HashMap<String, String>>,
    path_params: Option<HashMap<String, String>>,
}

impl EndpointBuilder {
    pub fn new() -> Self {
        EndpointBuilder::default()
    }

    pub fn base_url(mut self, base_url: &str) -> Self {
        self.base_url = Some(base_url.to_string());
        self
    }

    pub fn endpoint(mut self, endpoint: &str) -> Self {
        self.endpoint = Some(endpoint.to_string());
        self
    }

    pub fn method(mut self, method: Method) -> Self {
        self.method = Some(method);
        self
    }

    pub fn json_body(mut self, json_body: Value) -> Self {
        self.json_body = Some(json_body);
        self
    }

    pub fn query_params(mut self, query_params: HashMap<String, String>) -> Self {
        self.query_params = Some(query_params);
        self
    }

    pub fn path_params(mut self, path_params: HashMap<String, String>) -> Self {
        self.path_params = Some(path_params);
        self
    }

    pub fn build(self) -> Result<Endpoint, Box<dyn std::error::Error>> {
        Ok(Endpoint {
            base_url: self.base_url.ok_or("Base URL is required")?,
            endpoint: self.endpoint.ok_or("Endpoint is required")?,
            method: self.method.ok_or("Method is required")?,
            json_body: self.json_body,
            query_params: self.query_params,
            path_params: self.path_params,
        })
    }
}

pub struct Endpoint {
    base_url: String,
    endpoint: String,
    method: Method,
    json_body: Option<Value>,
    query_params: Option<HashMap<String, String>>,
    path_params: Option<HashMap<String, String>>,
}

impl Endpoint {
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder::new()
    }

    pub async fn send(self) -> Result<Value, Box<dyn std::error::Error>> {
        let client = Client::new();
        let mut url = Url::parse(&self.base_url)?;

        url.set_path(&self.endpoint);

        if let Some(params) = self.query_params {
            let mut serializer = url.query_pairs_mut();
            for (key, value) in params {
                serializer.append_pair(&key, &value);
            }
        }

        let mut request = client.request(self.method, url);

        if let Some(json) = self.json_body {
            request = request.json(&json);
        }

        let response = request.send().await?;

        if response.status().is_success() {
            match response.json::<Value>().await {
                Ok(json) => {
                    info!("Request SUCCESS: {:#?}", json);
                    Ok(json)
                }
                Err(e) => {
                    error!("Failed to parse JSON response: {:?}", e);
                    Err(Box::new(e))
                }
            }
        } else {
            let status = response.status();
            let error_text = response.text().await?;

            let re = Regex::new(r"\x1B\[[0-9;]*[mK]").unwrap();
            let cleaned_error_text = re.replace_all(&error_text, "").to_string();

            match serde_json::from_str::<Value>(&cleaned_error_text) {
                Ok(mut json) => {
                    if let Some(details) = json.get_mut("details") {
                        if let Some(details_str) = details.as_str() {
                            let cleaned_details = re.replace_all(details_str, "").to_string();
                            *details = Value::String(cleaned_details);
                        }
                    }

                    error!(
                        "Request FAILED with status {:?} and error: {:#?}",
                        status, json
                    );
                    Err(err!(AnyErr, "Request failed with error details").into())
                }
                Err(_) => {
                    error!(
                        "Request FAILED with status {:?} and error: {}",
                        status, cleaned_error_text
                    );
                    Err(err!(AnyErr, "Request failed with plain error text").into())
                }
            }
        }
    }
}
