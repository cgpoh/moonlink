#[cfg(any(feature = "storage-gcs", feature = "storage-s3"))]
use crate::MoonlinkSecretType;
use crate::MoonlinkTableSecret;
use serde::{Deserialize, Serialize};

#[cfg(feature = "storage-gcs")]
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WriteOption {
    /// Used to overwrite write option.
    #[serde(default)]
    pub multipart_upload_threshold: Option<usize>,
}

/// Default endpoint suffix for public Azure cloud.
#[cfg(feature = "storage-azdls")]
pub const AZURE_PUBLIC_CLOUD_ENDPOINT_SUFFIX: &str = "core.windows.net";
/// Default Microsoft Entra ID authority host for public Azure cloud, required by service principal authentication.
#[cfg(feature = "storage-azdls")]
const AZURE_PUBLIC_CLOUD_AUTHORITY_HOST: &str = "https://login.microsoftonline.com";

/// Environment variables read by [`AzdlsAuth::Environment`], following Azure SDK naming conventions.
#[cfg(feature = "storage-azdls")]
pub const AZURE_STORAGE_ACCOUNT_KEY_ENV: &str = "AZURE_STORAGE_ACCOUNT_KEY";
#[cfg(feature = "storage-azdls")]
pub const AZURE_STORAGE_SAS_TOKEN_ENV: &str = "AZURE_STORAGE_SAS_TOKEN";
#[cfg(feature = "storage-azdls")]
pub const AZURE_TENANT_ID_ENV: &str = "AZURE_TENANT_ID";
#[cfg(feature = "storage-azdls")]
pub const AZURE_CLIENT_ID_ENV: &str = "AZURE_CLIENT_ID";
#[cfg(feature = "storage-azdls")]
pub const AZURE_CLIENT_SECRET_ENV: &str = "AZURE_CLIENT_SECRET";
#[cfg(feature = "storage-azdls")]
pub const AZURE_AUTHORITY_HOST_ENV: &str = "AZURE_AUTHORITY_HOST";

/// Authentication method for Azure Data Lake Storage Gen2.
#[cfg(feature = "storage-azdls")]
#[derive(Clone, Deserialize, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AzdlsAuth {
    /// Storage account shared key.
    AccountKey { account_key: String },
    /// Shared access signature.
    SasToken { sas_token: String },
    /// Microsoft Entra ID service principal with client secret.
    ServicePrincipal {
        tenant_id: String,
        client_id: String,
        client_secret: String,
        /// Defaults to public Azure cloud authority host.
        #[serde(default)]
        authority_host: Option<String>,
    },
    /// Load credentials from environment variables at access time, with the same precedence as explicit methods:
    /// - [`AZURE_STORAGE_SAS_TOKEN_ENV`]
    /// - [`AZURE_STORAGE_ACCOUNT_KEY_ENV`]
    /// - [`AZURE_TENANT_ID_ENV`], [`AZURE_CLIENT_ID_ENV`], [`AZURE_CLIENT_SECRET_ENV`] and optional [`AZURE_AUTHORITY_HOST_ENV`]
    ///
    /// If none of them is set, falls back to managed identity, where [`AZURE_CLIENT_ID_ENV`] selects a user-assigned identity.
    Environment,
}

#[cfg(feature = "storage-azdls")]
impl std::fmt::Debug for AzdlsAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AzdlsAuth::AccountKey { .. } => f
                .debug_struct("AccountKey")
                .field("account_key", &"xxxxx")
                .finish(),
            AzdlsAuth::SasToken { .. } => f
                .debug_struct("SasToken")
                .field("sas_token", &"xxxxx")
                .finish(),
            AzdlsAuth::ServicePrincipal {
                tenant_id,
                client_id,
                authority_host,
                client_secret: _,
            } => f
                .debug_struct("ServicePrincipal")
                .field("tenant_id", tenant_id)
                .field("client_id", client_id)
                .field("client_secret", &"xxxxx")
                .field("authority_host", authority_host)
                .finish(),
            AzdlsAuth::Environment => f.write_str("Environment"),
        }
    }
}

/// Credentials resolved from [`AzdlsAuth`], unset fields are left for the underlying credential loader.
#[cfg(feature = "storage-azdls")]
#[derive(Debug, Default, PartialEq)]
pub(crate) struct AzdlsCredentials {
    pub(crate) account_key: Option<String>,
    pub(crate) sas_token: Option<String>,
    pub(crate) tenant_id: Option<String>,
    pub(crate) client_id: Option<String>,
    pub(crate) client_secret: Option<String>,
    pub(crate) authority_host: Option<String>,
}

#[cfg(feature = "storage-azdls")]
impl AzdlsAuth {
    /// Resolve credentials, reading process environment variables for [`AzdlsAuth::Environment`].
    pub(crate) fn resolve_credentials(&self) -> AzdlsCredentials {
        self.resolve_credentials_with(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
    }

    fn resolve_credentials_with(
        &self,
        get_env: impl Fn(&str) -> Option<String>,
    ) -> AzdlsCredentials {
        let mut credentials = match self {
            AzdlsAuth::AccountKey { account_key } => AzdlsCredentials {
                account_key: Some(account_key.clone()),
                ..Default::default()
            },
            AzdlsAuth::SasToken { sas_token } => AzdlsCredentials {
                sas_token: Some(sas_token.clone()),
                ..Default::default()
            },
            AzdlsAuth::ServicePrincipal {
                tenant_id,
                client_id,
                client_secret,
                authority_host,
            } => AzdlsCredentials {
                tenant_id: Some(tenant_id.clone()),
                client_id: Some(client_id.clone()),
                client_secret: Some(client_secret.clone()),
                authority_host: authority_host.clone(),
                ..Default::default()
            },
            AzdlsAuth::Environment => AzdlsCredentials {
                account_key: get_env(AZURE_STORAGE_ACCOUNT_KEY_ENV),
                sas_token: get_env(AZURE_STORAGE_SAS_TOKEN_ENV),
                tenant_id: get_env(AZURE_TENANT_ID_ENV),
                client_id: get_env(AZURE_CLIENT_ID_ENV),
                client_secret: get_env(AZURE_CLIENT_SECRET_ENV),
                authority_host: get_env(AZURE_AUTHORITY_HOST_ENV),
            },
        };
        // Service principal authentication is silently skipped without authority host.
        if credentials.client_secret.is_some() && credentials.authority_host.is_none() {
            credentials.authority_host = Some(AZURE_PUBLIC_CLOUD_AUTHORITY_HOST.to_string());
        }
        credentials
    }
}

/// Get ADLS Gen2 dfs endpoint host, i.e. "<account>.dfs.core.windows.net".
#[cfg(feature = "storage-azdls")]
pub(crate) fn get_azdls_host(account_name: &str, endpoint_suffix: &Option<String>) -> String {
    let endpoint_suffix = endpoint_suffix
        .as_deref()
        .unwrap_or(AZURE_PUBLIC_CLOUD_ENDPOINT_SUFFIX);
    format!("{account_name}.dfs.{endpoint_suffix}")
}

/// StorageConfig contains configuration for multiple storage backends.
#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub enum StorageConfig {
    #[cfg(feature = "storage-fs")]
    #[serde(rename = "fs")]
    FileSystem {
        root_directory: String,
        // Used for atomic write operation: write files to a temporary directory and rename.
        //
        // Caveat:
        // - Not every filesystem provides atomic [`rename`] semantics;
        // - Rename doesn't work across different devices.
        atomic_write_dir: Option<String>,
    },
    #[cfg(feature = "storage-s3")]
    #[serde(rename = "s3")]
    S3 {
        access_key_id: String,
        secret_access_key: String,
        region: String,
        bucket: String,
        #[serde(default)]
        endpoint: Option<String>,
    },
    #[cfg(feature = "storage-gcs")]
    #[serde(rename = "gcs")]
    Gcs {
        /// GCS project.
        project: String,
        /// GCS bucket region.
        region: String,
        /// GCS bucket.
        bucket: String,
        /// HMAC key and secret.
        access_key_id: String,
        secret_access_key: String,
        /// Used for fake GCS server.
        #[serde(default)]
        endpoint: Option<String>,
        /// Used for fake GCS server.
        #[serde(default)]
        disable_auth: bool,
        /// Write options, only overwrite if specified.
        #[serde(default)]
        write_option: Option<WriteOption>,
    },
    /// Azure Data Lake Storage Gen2, accessed via "abfss://<filesystem>@<account>.dfs.<endpoint_suffix>".
    #[cfg(feature = "storage-azdls")]
    #[serde(rename = "azdls")]
    Azdls {
        /// Storage account name.
        account_name: String,
        /// Filesystem (container) name.
        filesystem: String,
        /// Endpoint suffix, defaults to [`AZURE_PUBLIC_CLOUD_ENDPOINT_SUFFIX`]; set for sovereign clouds.
        #[serde(default)]
        endpoint_suffix: Option<String>,
        /// Authentication method.
        auth: AzdlsAuth,
    },
}

impl std::fmt::Debug for StorageConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem {
                root_directory,
                atomic_write_dir,
            } => f
                .debug_struct("FileSystem")
                .field("root_directory", root_directory)
                .field("atomic_write_dir", atomic_write_dir)
                .finish(),

            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 {
                region,
                bucket,
                endpoint,
                access_key_id: _,
                secret_access_key: _,
            } => f
                .debug_struct("S3")
                .field("region", region)
                .field("bucket", bucket)
                .field("endpoint", endpoint)
                .field("access key id", &"xxxxx")
                .field("secret access key", &"xxxxx")
                .finish(),

            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs {
                project,
                region,
                bucket,
                endpoint,
                disable_auth,
                write_option,
                access_key_id: _,
                secret_access_key: _,
            } => f
                .debug_struct("Gcs")
                .field("project", project)
                .field("region", region)
                .field("bucket", bucket)
                .field("endpoint", endpoint)
                .field("disable_auth", disable_auth)
                .field("write_option", write_option)
                .field("access key id", &"xxxxx")
                .field("secret access key", &"xxxxx")
                .finish(),

            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls {
                account_name,
                filesystem,
                endpoint_suffix,
                auth,
            } => f
                .debug_struct("Azdls")
                .field("account_name", account_name)
                .field("filesystem", filesystem)
                .field("endpoint_suffix", endpoint_suffix)
                .field("auth", auth)
                .finish(),
        }
    }
}

impl StorageConfig {
    /// Get root path for the given filesystem config.
    pub fn get_root_path(&self) -> String {
        match &self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem { root_directory, .. } => root_directory.to_string(),
            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs { bucket, .. } => format!("gs://{bucket}"),
            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 { bucket, .. } => format!("s3://{bucket}"),
            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls {
                account_name,
                filesystem,
                endpoint_suffix,
                ..
            } => format!(
                "abfss://{filesystem}@{}",
                get_azdls_host(account_name, endpoint_suffix)
            ),
        }
    }

    /// Get region for object storage config.
    pub fn get_region(&self) -> Option<String> {
        match &self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem { .. } => None,
            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs { region, .. } => Some(region.clone()),
            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 { region, .. } => Some(region.clone()),
            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls { .. } => None,
        }
    }

    /// Get access key id.
    pub fn get_access_key_id(&self) -> Option<String> {
        match &self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem { .. } => None,
            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs { access_key_id, .. } => Some(access_key_id.clone()),
            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 { access_key_id, .. } => Some(access_key_id.clone()),
            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls { .. } => None,
        }
    }

    /// Get secret access key.
    pub fn get_secret_access_key(&self) -> Option<String> {
        match &self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem { .. } => None,
            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs {
                secret_access_key, ..
            } => Some(secret_access_key.clone()),
            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 {
                secret_access_key, ..
            } => Some(secret_access_key.clone()),
            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls { .. } => None,
        }
    }

    /// Extract security metadata entry from current filesystem config.
    pub fn extract_security_metadata_entry(&self) -> Option<MoonlinkTableSecret> {
        match &self {
            #[cfg(feature = "storage-fs")]
            StorageConfig::FileSystem { .. } => None,
            #[cfg(feature = "storage-gcs")]
            StorageConfig::Gcs {
                project,
                region,
                access_key_id,
                secret_access_key,
                endpoint,
                ..
            } => Some(MoonlinkTableSecret {
                secret_type: MoonlinkSecretType::Gcs,
                key_id: access_key_id.to_string(),
                secret: secret_access_key.to_string(),
                project: Some(project.to_string()),
                endpoint: endpoint.clone(),
                region: Some(region.to_string()),
            }),
            #[cfg(feature = "storage-s3")]
            StorageConfig::S3 {
                access_key_id,
                secret_access_key,
                region,
                endpoint,
                ..
            } => Some(MoonlinkTableSecret {
                secret_type: MoonlinkSecretType::S3,
                key_id: access_key_id.to_string(),
                secret: secret_access_key.to_string(),
                project: None,
                endpoint: endpoint.clone(),
                region: Some(region.clone()),
            }),
            // Azure credentials don't fit the access key and secret model.
            #[cfg(feature = "storage-azdls")]
            StorageConfig::Azdls { .. } => None,
        }
    }
}

#[cfg(all(test, feature = "storage-gcs"))]
mod tests {
    use crate::StorageConfig;

    /// Testing scenario: deserialize storage config with partial GCS field populated.
    #[test]
    fn test_deserialize_storage_config_with_only_necessary() {
        let json = r#"
        {
            "gcs": {
                "project": "test-project",
                "region": "us-west1",
                "bucket": "test-bucket",
                "access_key_id": "fake-access-key",
                "secret_access_key": "fake-secret-key"
            }
        }
        "#;

        let parsed_config: StorageConfig = serde_json::from_str(json).unwrap();
        assert_eq!(
            parsed_config,
            StorageConfig::Gcs {
                project: "test-project".to_string(),
                region: "us-west1".to_string(),
                bucket: "test-bucket".to_string(),
                access_key_id: "fake-access-key".to_string(),
                secret_access_key: "fake-secret-key".to_string(),
                endpoint: None,
                disable_auth: false,
                write_option: None,
            }
        );
    }
}

#[cfg(all(test, feature = "storage-azdls"))]
mod azdls_tests {
    use super::*;
    use std::collections::HashMap;

    fn parse_auth(json: &str) -> AzdlsAuth {
        let config: StorageConfig = serde_json::from_str(&format!(
            r#"{{"azdls": {{"account_name": "acct", "filesystem": "fs", "auth": {json}}}}}"#
        ))
        .unwrap();
        match config {
            StorageConfig::Azdls { auth, .. } => auth,
            _ => unreachable!(),
        }
    }

    #[test]
    fn test_deserialize_auth_methods() {
        assert_eq!(
            parse_auth(r#"{"type": "account_key", "account_key": "key"}"#),
            AzdlsAuth::AccountKey {
                account_key: "key".to_string()
            }
        );
        assert_eq!(
            parse_auth(r#"{"type": "sas_token", "sas_token": "sv=2024"}"#),
            AzdlsAuth::SasToken {
                sas_token: "sv=2024".to_string()
            }
        );
        assert_eq!(
            parse_auth(
                r#"{"type": "service_principal", "tenant_id": "t", "client_id": "c", "client_secret": "s"}"#
            ),
            AzdlsAuth::ServicePrincipal {
                tenant_id: "t".to_string(),
                client_id: "c".to_string(),
                client_secret: "s".to_string(),
                authority_host: None,
            }
        );
        assert_eq!(
            parse_auth(r#"{"type": "environment"}"#),
            AzdlsAuth::Environment
        );
    }

    #[test]
    fn test_root_path() {
        let auth = AzdlsAuth::Environment;
        let config = StorageConfig::Azdls {
            account_name: "acct".to_string(),
            filesystem: "fs".to_string(),
            endpoint_suffix: None,
            auth: auth.clone(),
        };
        assert_eq!(
            config.get_root_path(),
            "abfss://fs@acct.dfs.core.windows.net"
        );

        let config = StorageConfig::Azdls {
            account_name: "acct".to_string(),
            filesystem: "fs".to_string(),
            endpoint_suffix: Some("core.chinacloudapi.cn".to_string()),
            auth,
        };
        assert_eq!(
            config.get_root_path(),
            "abfss://fs@acct.dfs.core.chinacloudapi.cn"
        );
    }

    #[test]
    fn test_resolve_explicit_credentials() {
        let auth = AzdlsAuth::ServicePrincipal {
            tenant_id: "t".to_string(),
            client_id: "c".to_string(),
            client_secret: "s".to_string(),
            authority_host: None,
        };
        // Authority host defaults to public cloud, otherwise service principal authentication is skipped.
        assert_eq!(
            auth.resolve_credentials_with(|_| unreachable!()),
            AzdlsCredentials {
                tenant_id: Some("t".to_string()),
                client_id: Some("c".to_string()),
                client_secret: Some("s".to_string()),
                authority_host: Some(AZURE_PUBLIC_CLOUD_AUTHORITY_HOST.to_string()),
                ..Default::default()
            }
        );

        let auth = AzdlsAuth::AccountKey {
            account_key: "key".to_string(),
        };
        assert_eq!(
            auth.resolve_credentials_with(|_| unreachable!()),
            AzdlsCredentials {
                account_key: Some("key".to_string()),
                ..Default::default()
            }
        );
    }

    #[test]
    fn test_resolve_environment_credentials() {
        let auth = AzdlsAuth::Environment;

        // Service principal from environment.
        let envs = HashMap::from([
            (AZURE_TENANT_ID_ENV, "t"),
            (AZURE_CLIENT_ID_ENV, "c"),
            (AZURE_CLIENT_SECRET_ENV, "s"),
        ]);
        assert_eq!(
            auth.resolve_credentials_with(|key| envs.get(key).map(|v| v.to_string())),
            AzdlsCredentials {
                tenant_id: Some("t".to_string()),
                client_id: Some("c".to_string()),
                client_secret: Some("s".to_string()),
                authority_host: Some(AZURE_PUBLIC_CLOUD_AUTHORITY_HOST.to_string()),
                ..Default::default()
            }
        );

        // Client id only, used to select user-assigned managed identity; no authority host injected.
        let envs = HashMap::from([(AZURE_CLIENT_ID_ENV, "c")]);
        assert_eq!(
            auth.resolve_credentials_with(|key| envs.get(key).map(|v| v.to_string())),
            AzdlsCredentials {
                client_id: Some("c".to_string()),
                ..Default::default()
            }
        );

        // Nothing set, falls back to managed identity.
        assert_eq!(
            auth.resolve_credentials_with(|_| None),
            AzdlsCredentials::default()
        );
    }

    #[test]
    fn test_debug_redacts_secrets() {
        let config = StorageConfig::Azdls {
            account_name: "acct".to_string(),
            filesystem: "fs".to_string(),
            endpoint_suffix: None,
            auth: AzdlsAuth::ServicePrincipal {
                tenant_id: "t".to_string(),
                client_id: "c".to_string(),
                client_secret: "super-secret".to_string(),
                authority_host: None,
            },
        };
        assert!(!format!("{config:?}").contains("super-secret"));

        let auth = AzdlsAuth::AccountKey {
            account_key: "super-secret".to_string(),
        };
        assert!(!format!("{auth:?}").contains("super-secret"));
    }
}
