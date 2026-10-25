//! Tencent Cloud COS backend.
//!
//! The browser uploads directly to a private COS bucket with a signed PUT URL.
//! The server only sends a HEAD request to verify the uploaded object's size and
//! sha256 metadata, and returns signed GET URLs for downloads.

use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use hmac::{Hmac, KeyInit, Mac};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::StatusCode;
use sha1::{Digest, Sha1};

use super::{Receipt, StorageBackend, UploadTarget};
use crate::error::{Error, Result, bad};
use crate::model::Location;

const PATH_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

// COS canonical query/header values use RFC 3986 encoding. In particular,
// host names and metadata values keep `-`, `.`, `_`, and `~` unchanged.
const COMPONENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

#[derive(Clone)]
pub struct CosConfig {
    pub secret_id: String,
    pub secret_key: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub signed_url_ttl: u64,
    pub upload_url_ttl: u64,
}

pub struct CosBackend {
    cfg: CosConfig,
    http: reqwest::Client,
}

impl CosBackend {
    pub fn new(cfg: CosConfig, http: reqwest::Client) -> CosBackend {
        CosBackend { cfg, http }
    }

    fn endpoint_host(&self) -> &str {
        self.cfg
            .endpoint
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .split('/')
            .next()
            .unwrap_or("")
    }

    fn object_url(&self, key: &str) -> String {
        format!(
            "{}/{}",
            self.cfg.endpoint.trim_end_matches('/'),
            encode_path(key)
        )
    }

    fn key_of(loc: &Location) -> Result<&str> {
        match loc {
            Location::Cos { key } => Ok(key),
            _ => Err(bad("not a COS location")),
        }
    }

    fn sha256_from_key(key: &str) -> Result<&str> {
        let hash = key
            .strip_prefix("blobs/")
            .ok_or_else(|| bad("bad COS object key"))?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad("bad COS object key"));
        }
        Ok(hash)
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn sign_url(&self, method: &str, key: &str, headers: &[(&str, &str)], ttl: u64) -> String {
        // Match Tencent's SDK signing window: allow a small amount of clock
        // skew before the request reaches COS.
        let start = Self::now().saturating_sub(60);
        let end = start.saturating_add(ttl.max(60));
        let key_time = format!("{start};{end}");
        let host = self.endpoint_host();
        let uri = format!("/{}", encode_path(key));

        let mut canonical_headers: Vec<(String, String)> = headers
            .iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), encode_component(value)))
            .collect();
        canonical_headers.push(("host".into(), encode_component(host)));
        canonical_headers.sort_by(|a, b| a.0.cmp(&b.0));
        let header_list = canonical_headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let header_text = canonical_headers
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("&");
        let http_string = format!("{}\n{uri}\n\n{header_text}\n", method.to_ascii_lowercase());
        let http_string_sha1 = hex::encode(Sha1::digest(http_string.as_bytes()));
        let string_to_sign = format!("sha1\n{key_time}\n{http_string_sha1}\n");

        let mut sign_key = Hmac::<Sha1>::new_from_slice(self.cfg.secret_key.as_bytes())
            .expect("HMAC accepts every key length");
        sign_key.update(key_time.as_bytes());
        // COS derives the second HMAC from the hexadecimal text of the first
        // HMAC, as implemented by the official SDK, rather than its raw bytes.
        let sign_key = hex::encode(sign_key.finalize().into_bytes());
        let mut signature = Hmac::<Sha1>::new_from_slice(sign_key.as_bytes())
            .expect("HMAC accepts every key length");
        signature.update(string_to_sign.as_bytes());
        let signature = hex::encode(signature.finalize().into_bytes());

        let auth = format!(
            "q-sign-algorithm=sha1&q-ak={}&q-sign-time={key_time}&q-key-time={key_time}&q-header-list={header_list}&q-url-param-list=&q-signature={signature}",
            encode_component(&self.cfg.secret_id),
        );
        format!("{}?{auth}", self.object_url(key))
    }
}

#[async_trait]
impl StorageBackend for CosBackend {
    fn name(&self) -> &'static str {
        "cos"
    }

    async fn reserve(&self, _display_name: &str, _size: u64, sha256: &str) -> Result<Location> {
        if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad("bad sha256"));
        }
        Ok(Location::Cos {
            key: format!("blobs/{sha256}"),
        })
    }

    fn upload_target(&self, reserved: &Location, _size: u64) -> Result<UploadTarget> {
        let key = Self::key_of(reserved)?;
        let sha256 = Self::sha256_from_key(key)?;
        let url = self.sign_url(
            "PUT",
            key,
            &[("x-cos-meta-sha256", sha256)],
            self.cfg.upload_url_ttl,
        );
        Ok(UploadTarget {
            url,
            method: "PUT",
            headers: vec![("x-cos-meta-sha256".into(), sha256.into())],
        })
    }

    async fn confirm(
        &self,
        reserved: &Location,
        size: u64,
        sha256: &str,
        _receipt: &Receipt,
    ) -> Result<Location> {
        let key = Self::key_of(reserved)?;
        if Self::sha256_from_key(key)? != sha256 {
            return Err(Error::Conflict("COS 对象地址与文件校验值不一致".into()));
        }
        let url = self.sign_url("HEAD", key, &[], self.cfg.signed_url_ttl);
        let response = self.http.head(url).send().await?;
        if response.status() == StatusCode::NOT_FOUND {
            return Err(Error::Conflict("COS 中还没有收到文件，请重试".into()));
        }
        if !response.status().is_success() {
            return Err(Error::Upstream(format!(
                "COS HEAD failed: HTTP {}",
                response.status()
            )));
        }
        let actual_size = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        if actual_size != Some(size) {
            return Err(Error::Conflict("COS 文件大小与声明不符".into()));
        }
        let actual_sha256 = response
            .headers()
            .get("x-cos-meta-sha256")
            .and_then(|v| v.to_str().ok());
        if actual_sha256 != Some(sha256) {
            return Err(Error::Conflict("COS 文件校验值与声明不符".into()));
        }
        Ok(reserved.clone())
    }

    fn download_urls(&self, loc: &Location) -> Vec<String> {
        let Ok(key) = Self::key_of(loc) else {
            return Vec::new();
        };
        vec![self.sign_url("GET", key, &[], self.cfg.signed_url_ttl)]
    }
}

fn encode_component(value: &str) -> String {
    utf8_percent_encode(value, COMPONENT_ENCODE_SET).to_string()
}

fn encode_path(path: &str) -> String {
    utf8_percent_encode(path, PATH_ENCODE_SET).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn backend() -> CosBackend {
        CosBackend::new(
            CosConfig {
                secret_id: "AKIDexample".into(),
                secret_key: "secret".into(),
                bucket: "materials-123".into(),
                region: "ap-beijing".into(),
                endpoint: "https://materials-123.cos.ap-beijing.myqcloud.com".into(),
                signed_url_ttl: 1800,
                upload_url_ttl: 3600,
            },
            reqwest::Client::new(),
        )
    }

    #[test]
    fn signs_private_object_urls_without_exposing_secret_key() {
        let b = backend();
        let url = b.sign_url("GET", "blobs/0123456789abcdef", &[], 1800);
        assert!(url.starts_with("https://materials-123.cos.ap-beijing.myqcloud.com/blobs/"));
        assert!(url.contains("q-signature="));
        assert!(!url.contains("secret"));
    }

    #[test]
    fn upload_target_signs_sha256_metadata() {
        let b = backend();
        let sha = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let target = b
            .upload_target(
                &Location::Cos {
                    key: format!("blobs/{sha}"),
                },
                10,
            )
            .unwrap();
        assert_eq!(target.method, "PUT");
        assert_eq!(
            target.headers,
            vec![("x-cos-meta-sha256".into(), sha.into())]
        );
        assert!(target.url.contains("q-header-list=host;x-cos-meta-sha256"));
    }

    #[test]
    fn canonical_components_keep_rfc3986_unreserved_bytes() {
        assert_eq!(encode_component("host-name.example_~"), "host-name.example_~");
        assert_eq!(encode_component("a b"), "a%20b");
    }
}
