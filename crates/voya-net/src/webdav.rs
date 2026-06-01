use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use quick_xml::{events::Event, Reader};
use reqwest::{Client, Method, StatusCode};
use thiserror::Error;

const DEFAULT_BACKUP_DIR: &str = "VoyaVPN_backup";
const DEFAULT_BACKUP_FILE: &str = "backup.zip";
const TEST_FILE: &str = "readme_test";
const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'/')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'}');

pub type Result<T> = std::result::Result<T, WebDavError>;

#[derive(Debug, Error)]
pub enum WebDavError {
    #[error("WebDAV URL is required")]
    MissingUrl,
    #[error("WebDAV username is required")]
    MissingUsername,
    #[error("WebDAV password is required")]
    MissingPassword,
    #[error("failed to build WebDAV request {method} {url}: {source}")]
    RequestBuild {
        method: String,
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("WebDAV request failed {method} {url}: {source}")]
    Request {
        method: String,
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("WebDAV request {method} {url} returned HTTP {status}: {body}")]
    Status {
        method: String,
        url: String,
        status: StatusCode,
        body: String,
    },
    #[error("failed to parse WebDAV XML: {0}")]
    Xml(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebDavConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub dir_name: Option<String>,
}

impl WebDavConfig {
    pub fn new(
        url: impl Into<String>,
        username: impl Into<String>,
        password: impl Into<String>,
        dir_name: Option<String>,
    ) -> Result<Self> {
        let url = url.into().trim().trim_end_matches('/').to_string();
        let username = username.into().trim().to_string();
        let password = password.into();

        if url.is_empty() {
            return Err(WebDavError::MissingUrl);
        }
        if username.is_empty() {
            return Err(WebDavError::MissingUsername);
        }
        if password.is_empty() {
            return Err(WebDavError::MissingPassword);
        }

        Ok(Self {
            url,
            username,
            password,
            dir_name: dir_name.and_then(|value| {
                let trimmed = value.trim().trim_matches('/').to_string();
                (!trimmed.is_empty()).then_some(trimmed)
            }),
        })
    }

    #[must_use]
    pub fn collection_name(&self) -> &str {
        self.dir_name.as_deref().unwrap_or(DEFAULT_BACKUP_DIR)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebDavEntry {
    pub href: String,
    pub display_name: Option<String>,
    pub content_length: Option<u64>,
    pub is_collection: bool,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebDavTransferOutcome {
    pub remote_path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone)]
pub struct WebDavClient {
    client: Client,
    config: WebDavConfig,
}

impl WebDavClient {
    #[must_use]
    pub fn new(config: WebDavConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }

    #[must_use]
    pub fn config(&self) -> &WebDavConfig {
        &self.config
    }

    pub async fn check_connection(&self) -> Result<()> {
        self.ensure_collection().await?;
        let probe = join_remote_path(self.config.collection_name(), TEST_FILE);
        self.upload(&probe, TEST_FILE.as_bytes().to_vec()).await?;
        self.delete(&probe).await?;

        Ok(())
    }

    pub async fn list_collection(&self) -> Result<Vec<WebDavEntry>> {
        self.propfind(self.config.collection_name()).await
    }

    pub async fn ensure_collection(&self) -> Result<()> {
        let path = self.config.collection_name();
        let url = self.url(path);
        let method = "MKCOL";
        let request = self
            .client
            .request(custom_method(method), &url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .build()
            .map_err(|source| WebDavError::RequestBuild {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;
        let response =
            self.client
                .execute(request)
                .await
                .map_err(|source| WebDavError::Request {
                    method: method.to_string(),
                    url: url.clone(),
                    source,
                })?;
        let status = response.status();

        if status.is_success()
            || status == StatusCode::METHOD_NOT_ALLOWED
            || status == StatusCode::CONFLICT
        {
            return Ok(());
        }

        Err(status_error(method, &url, response).await)
    }

    pub async fn upload_backup(&self, body: Vec<u8>) -> Result<WebDavTransferOutcome> {
        self.ensure_collection().await?;
        let remote_path = self.backup_remote_path();
        self.upload(&remote_path, body).await
    }

    pub async fn download_backup(&self) -> Result<Vec<u8>> {
        self.download(&self.backup_remote_path()).await
    }

    pub async fn delete_backup(&self) -> Result<()> {
        self.delete(&self.backup_remote_path()).await
    }

    pub async fn propfind(&self, path: &str) -> Result<Vec<WebDavEntry>> {
        let url = self.url(path);
        let method = "PROPFIND";
        let body = r#"<?xml version="1.0" encoding="utf-8" ?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:displayname />
    <d:getcontentlength />
    <d:getlastmodified />
    <d:resourcetype />
  </d:prop>
</d:propfind>"#;
        let request = self
            .client
            .request(custom_method(method), &url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .header("Depth", "1")
            .header("Content-Type", "application/xml; charset=utf-8")
            .body(body)
            .build()
            .map_err(|source| WebDavError::RequestBuild {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;
        let response =
            self.client
                .execute(request)
                .await
                .map_err(|source| WebDavError::Request {
                    method: method.to_string(),
                    url: url.clone(),
                    source,
                })?;
        let status = response.status();
        if !status.is_success() && status.as_u16() != 207 {
            return Err(status_error(method, &url, response).await);
        }
        let text = response
            .text()
            .await
            .map_err(|source| WebDavError::Request {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;

        parse_propfind_response(&text)
    }

    pub async fn upload(&self, path: &str, body: Vec<u8>) -> Result<WebDavTransferOutcome> {
        let url = self.url(path);
        let method = "PUT";
        let bytes = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let response = self
            .client
            .put(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .body(body)
            .send()
            .await
            .map_err(|source| WebDavError::Request {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;
        if response.status().is_success() {
            return Ok(WebDavTransferOutcome {
                remote_path: path.to_string(),
                bytes,
            });
        }

        Err(status_error(method, &url, response).await)
    }

    pub async fn download(&self, path: &str) -> Result<Vec<u8>> {
        let url = self.url(path);
        let method = "GET";
        let response = self
            .client
            .get(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .map_err(|source| WebDavError::Request {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;
        if !response.status().is_success() {
            return Err(status_error(method, &url, response).await);
        }

        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|source| WebDavError::Request {
                method: method.to_string(),
                url,
                source,
            })
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let url = self.url(path);
        let method = "DELETE";
        let response = self
            .client
            .delete(&url)
            .basic_auth(&self.config.username, Some(&self.config.password))
            .send()
            .await
            .map_err(|source| WebDavError::Request {
                method: method.to_string(),
                url: url.clone(),
                source,
            })?;
        if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }

        Err(status_error(method, &url, response).await)
    }

    fn backup_remote_path(&self) -> String {
        join_remote_path(self.config.collection_name(), DEFAULT_BACKUP_FILE)
    }

    fn url(&self, path: &str) -> String {
        let path = encode_remote_path(path);
        if path.is_empty() {
            self.config.url.clone()
        } else {
            format!("{}/{}", self.config.url, path)
        }
    }
}

pub fn parse_propfind_response(xml: &str) -> Result<Vec<WebDavEntry>> {
    #[derive(Default)]
    struct EntryBuilder {
        href: String,
        display_name: Option<String>,
        content_length: Option<u64>,
        is_collection: bool,
        last_modified: Option<String>,
    }

    #[derive(Clone, Copy)]
    enum TextField {
        Href,
        DisplayName,
        ContentLength,
        LastModified,
    }

    let mut reader = Reader::from_str(xml);
    reader.trim_text(true);
    let mut buf = Vec::new();
    let mut field = None;
    let mut current: Option<EntryBuilder> = None;
    let mut entries = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) => match local_name(event.name().as_ref()) {
                "response" => current = Some(EntryBuilder::default()),
                "href" => field = Some(TextField::Href),
                "displayname" => field = Some(TextField::DisplayName),
                "getcontentlength" => field = Some(TextField::ContentLength),
                "getlastmodified" => field = Some(TextField::LastModified),
                "collection" => {
                    if let Some(entry) = current.as_mut() {
                        entry.is_collection = true;
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(event)) => {
                if local_name(event.name().as_ref()) == "collection" {
                    if let Some(entry) = current.as_mut() {
                        entry.is_collection = true;
                    }
                }
            }
            Ok(Event::Text(event)) => {
                if let (Some(field), Some(entry)) = (field, current.as_mut()) {
                    let text = event
                        .unescape()
                        .map_err(|source| WebDavError::Xml(source.to_string()))?
                        .into_owned();
                    match field {
                        TextField::Href => entry.href = text,
                        TextField::DisplayName => entry.display_name = non_empty(text),
                        TextField::ContentLength => {
                            entry.content_length = text.trim().parse::<u64>().ok();
                        }
                        TextField::LastModified => entry.last_modified = non_empty(text),
                    }
                }
            }
            Ok(Event::End(event)) => match local_name(event.name().as_ref()) {
                "response" => {
                    if let Some(entry) = current.take() {
                        entries.push(WebDavEntry {
                            href: entry.href,
                            display_name: entry.display_name,
                            content_length: entry.content_length,
                            is_collection: entry.is_collection,
                            last_modified: entry.last_modified,
                        });
                    }
                }
                "href" | "displayname" | "getcontentlength" | "getlastmodified" => {
                    field = None;
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(error) => return Err(WebDavError::Xml(error.to_string())),
            _ => {}
        }
        buf.clear();
    }

    Ok(entries)
}

fn custom_method(method: &str) -> Method {
    Method::from_bytes(method.as_bytes()).unwrap_or(Method::GET)
}

async fn status_error(method: &str, url: &str, response: reqwest::Response) -> WebDavError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    WebDavError::Status {
        method: method.to_string(),
        url: url.to_string(),
        status,
        body,
    }
}

fn join_remote_path(dir: &str, file: &str) -> String {
    [dir, file]
        .into_iter()
        .flat_map(|part| part.split('/'))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

fn encode_remote_path(path: &str) -> String {
    path.split('/')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| utf8_percent_encode(part, PATH_SEGMENT_ENCODE_SET).to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn local_name(name: &[u8]) -> &str {
    let bytes = name
        .iter()
        .position(|byte| *byte == b':')
        .map_or(name, |index| &name[index + 1..]);

    std::str::from_utf8(bytes).unwrap_or("")
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex as StdMutex},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    #[test]
    fn webdav_propfind_xml_fixture_parses_multistatus_entries() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/dav/VoyaVPN_backup/</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>VoyaVPN_backup</d:displayname>
        <d:resourcetype><d:collection /></d:resourcetype>
      </d:prop>
    </d:propstat>
  </d:response>
  <d:response>
    <d:href>/dav/VoyaVPN_backup/backup.zip</d:href>
    <d:propstat>
      <d:prop>
        <d:displayname>backup.zip</d:displayname>
        <d:getcontentlength>42</d:getcontentlength>
        <d:getlastmodified>Mon, 01 Jun 2026 00:00:00 GMT</d:getlastmodified>
        <d:resourcetype />
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let entries = parse_propfind_response(xml).expect("propfind XML");

        assert_eq!(entries.len(), 2);
        assert!(entries[0].is_collection);
        assert_eq!(entries[1].display_name.as_deref(), Some("backup.zip"));
        assert_eq!(entries[1].content_length, Some(42));
        assert!(!entries[1].is_collection);
    }

    #[tokio::test]
    async fn webdav_client_propfind_upload_download_and_delete_use_fixture_http() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let base = spawn_webdav_fixture(Arc::clone(&requests)).await;
        let config = WebDavConfig::new(base, "user", "pass", Some("VoyaVPN_backup".to_string()))
            .expect("config");
        let client = WebDavClient::new(config);

        let entries = client.list_collection().await.expect("propfind");
        assert_eq!(entries[0].display_name.as_deref(), Some("backup.zip"));

        let uploaded = client
            .upload_backup(b"zip-bytes".to_vec())
            .await
            .expect("upload");
        assert_eq!(uploaded.remote_path, "VoyaVPN_backup/backup.zip");

        let downloaded = client.download_backup().await.expect("download");
        assert_eq!(downloaded, b"zip-bytes");

        client.delete_backup().await.expect("delete");

        let seen = requests.lock().expect("requests").clone();
        assert!(seen.contains(&"PROPFIND /VoyaVPN_backup".to_string()));
        assert!(seen.contains(&"MKCOL /VoyaVPN_backup".to_string()));
        assert!(seen.contains(&"PUT /VoyaVPN_backup/backup.zip".to_string()));
        assert!(seen.contains(&"GET /VoyaVPN_backup/backup.zip".to_string()));
        assert!(seen.contains(&"DELETE /VoyaVPN_backup/backup.zip".to_string()));
    }

    async fn spawn_webdav_fixture(requests: Arc<StdMutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let routes = Arc::new(HashMap::from([
            (
                ("PROPFIND".to_string(), "/VoyaVPN_backup".to_string()),
                (
                    "207 Multi-Status".to_string(),
                    "application/xml".to_string(),
                    r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/VoyaVPN_backup/backup.zip</d:href><d:propstat><d:prop><d:displayname>backup.zip</d:displayname><d:getcontentlength>9</d:getcontentlength></d:prop></d:propstat></d:response></d:multistatus>"#.as_bytes().to_vec(),
                ),
            ),
            (
                ("MKCOL".to_string(), "/VoyaVPN_backup".to_string()),
                ("201 Created".to_string(), "text/plain".to_string(), Vec::new()),
            ),
            (
                ("PUT".to_string(), "/VoyaVPN_backup/backup.zip".to_string()),
                ("201 Created".to_string(), "text/plain".to_string(), Vec::new()),
            ),
            (
                ("GET".to_string(), "/VoyaVPN_backup/backup.zip".to_string()),
                (
                    "200 OK".to_string(),
                    "application/zip".to_string(),
                    b"zip-bytes".to_vec(),
                ),
            ),
            (
                ("DELETE".to_string(), "/VoyaVPN_backup/backup.zip".to_string()),
                ("204 No Content".to_string(), "text/plain".to_string(), Vec::new()),
            ),
        ]));

        tokio::spawn(async move {
            for _ in 0..5 {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let requests = Arc::clone(&requests);
                tokio::spawn(async move {
                    let mut buffer = vec![0; 8192];
                    let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..bytes_read]);
                    let first = request.lines().next().unwrap_or_default();
                    let mut parts = first.split_whitespace();
                    let method = parts.next().unwrap_or_default().to_string();
                    let path = parts.next().unwrap_or_default().to_string();
                    requests
                        .lock()
                        .expect("requests")
                        .push(format!("{method} {path}"));
                    let (status, content_type, body) =
                        routes.get(&(method, path)).cloned().unwrap_or_else(|| {
                            (
                                "404 Not Found".to_string(),
                                "text/plain".to_string(),
                                b"not found".to_vec(),
                            )
                        });
                    let response = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    let _ = socket.write_all(&body).await;
                });
            }
        });

        format!("http://{address}")
    }
}
