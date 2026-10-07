#[cfg(all(feature = "catalog-glue", feature = "storage-s3"))]
use crate::storage::table::iceberg::cloud_security_config::CloudSecurityConfig;
use crate::{storage::filesystem::accessor_config::AccessorConfig, StorageConfig};
use serde::{Deserialize, Serialize};
#[cfg(feature = "catalog-rest")]
use std::collections::HashMap;

#[cfg(feature = "catalog-rest")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RestCatalogConfig {
    #[serde(rename = "name")]
    #[serde(default)]
    pub name: String,

    #[serde(rename = "uri")]
    #[serde(default)]
    pub uri: String,

    /// Warehouse identifier sent to the REST catalog server.
    /// Some servers (i.e. iceberg-rest-fixture) take a storage URI, while others (i.e. Apache Polaris) take a logical catalog name.
    #[serde(rename = "warehouse")]
    #[serde(default)]
    pub warehouse: String,

    /// Storage URI under which iceberg tables are created, i.e. "s3://bucket/path".
    /// Required when [`warehouse`] is a logical name rather than a storage URI (i.e. Apache Polaris); defaults to [`warehouse`] if unset.
    #[serde(rename = "warehouse_location")]
    #[serde(default)]
    pub warehouse_location: Option<String>,

    /// Optional configuration properties.
    /// Unknown configs will be ignored.
    ///
    /// - prefix:          Optional URL path prefix to insert after the base URI and API version.
    /// - oauth2-server-uri: Custom OAuth2 server URI. Defaults to: [uri, PATH_V1:"v1", "oauth", "tokens"].join("/")
    /// - token:           Static authentication token used by the client for sending requests.
    /// - credential:      Client credentials used to fetch a new token, in the format of "<client_id>:<client_secret>" or "<client_secret>".
    /// - scope:           OAuth2 scope to request, defaults to "catalog". Apache Polaris requires "PRINCIPAL_ROLE:<role>" or "PRINCIPAL_ROLE:ALL".
    /// - header.<name>:   Extra HTTP header attached to every request.
    #[serde(rename = "props")]
    #[serde(default)]
    pub props: HashMap<String, String>,
}

#[cfg(feature = "catalog-rest")]
impl RestCatalogConfig {
    /// Get the storage URI under which iceberg tables are created.
    pub fn get_warehouse_location(&self) -> String {
        self.warehouse_location
            .clone()
            .unwrap_or_else(|| self.warehouse.clone())
    }
}

#[cfg(all(feature = "catalog-glue", feature = "storage-s3"))]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlueCatalogConfig {
    /// ========================
    /// AWS security configs.
    /// ========================
    ///
    #[serde(rename = "cloud_secret_config")]
    pub cloud_secret_config: CloudSecurityConfig,

    /// ========================
    /// Glue properties
    /// ========================
    ///
    #[serde(rename = "name")]
    #[serde(default)]
    pub name: String,

    /// Glue catalog URI.
    #[serde(rename = "uri")]
    #[serde(default)]
    pub uri: String,

    #[serde(rename = "catalog_id")]
    #[serde(default)]
    pub catalog_id: Option<String>,

    /// Notice, it should match data access config.
    #[serde(rename = "warehouse")]
    #[serde(default)]
    pub warehouse: String,

    /// If unassigned (default option), use https://s3.{s3_region}.amazonaws.com as endpoint.
    #[serde(rename = "s3_endpoint")]
    #[serde(default)]
    pub s3_endpoint: Option<String>,
}

pub type FileCatalogConfig = AccessorConfig;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum IcebergCatalogConfig {
    #[serde(rename = "file")]
    File { accessor_config: FileCatalogConfig },

    #[cfg(feature = "catalog-rest")]
    #[serde(rename = "rest")]
    Rest {
        rest_catalog_config: RestCatalogConfig,
    },

    #[cfg(all(feature = "catalog-glue", feature = "storage-s3"))]
    #[serde(rename = "glue")]
    Glue {
        glue_catalog_config: GlueCatalogConfig,
    },
}

impl IcebergCatalogConfig {
    pub fn get_warehouse_uri(&self) -> String {
        match self {
            IcebergCatalogConfig::File { accessor_config } => accessor_config.get_root_path(),
            #[cfg(feature = "catalog-rest")]
            IcebergCatalogConfig::Rest {
                rest_catalog_config,
            } => rest_catalog_config.get_warehouse_location(),
            #[cfg(all(feature = "catalog-glue", feature = "storage-s3"))]
            IcebergCatalogConfig::Glue {
                glue_catalog_config,
            } => glue_catalog_config.warehouse.clone(),
        }
    }

    pub fn get_file_catalog_accessor_config(&self) -> Option<FileCatalogConfig> {
        match self {
            IcebergCatalogConfig::File { accessor_config } => Some(accessor_config.clone()),
            #[cfg(any(feature = "catalog-rest", feature = "catalog-glue"))]
            _ => None,
        }
    }

    #[cfg(feature = "catalog-rest")]
    pub fn get_rest_catalog_config(&self) -> Option<RestCatalogConfig> {
        if let IcebergCatalogConfig::Rest {
            rest_catalog_config,
        } = self
        {
            return Some(rest_catalog_config.clone());
        }
        None
    }

    #[cfg(all(feature = "catalog-glue", feature = "storage-s3"))]
    pub fn get_glue_catalog_config(&self) -> Option<GlueCatalogConfig> {
        if let IcebergCatalogConfig::Glue {
            glue_catalog_config,
        } = self
        {
            return Some(glue_catalog_config.clone());
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IcebergTableConfig {
    /// Namespace for the iceberg table.
    pub namespace: Vec<String>,
    /// Iceberg table name.
    pub table_name: String,
    /// Accessor config for accessing data files.
    pub data_accessor_config: AccessorConfig,
    /// Catalog configuration (defaults to File).
    pub metadata_accessor_config: IcebergCatalogConfig,
}

impl IcebergTableConfig {
    const DEFAULT_WAREHOUSE_URI: &str = "/tmp/moonlink_iceberg";
    const DEFAULT_NAMESPACE: &str = "namespace";
    const DEFAULT_TABLE: &str = "table";
}

impl Default for IcebergTableConfig {
    fn default() -> Self {
        let storage_config = StorageConfig::FileSystem {
            root_directory: Self::DEFAULT_WAREHOUSE_URI.to_string(),
            // There's only one iceberg writer per-table, no need for atomic write feature.
            atomic_write_dir: None,
        };
        Self {
            namespace: vec![Self::DEFAULT_NAMESPACE.to_string()],
            table_name: Self::DEFAULT_TABLE.to_string(),
            data_accessor_config: AccessorConfig::new_with_storage_config(storage_config.clone()),
            metadata_accessor_config: IcebergCatalogConfig::File {
                accessor_config: AccessorConfig::new_with_storage_config(storage_config),
            },
        }
    }
}

#[cfg(all(test, feature = "catalog-rest"))]
mod tests {
    use super::*;

    #[test]
    fn test_rest_catalog_warehouse_location() {
        // Warehouse location falls back to warehouse when unset.
        let config: RestCatalogConfig =
            serde_json::from_str(r#"{"name":"c","uri":"http://h:8181","warehouse":"s3://b/p"}"#)
                .unwrap();
        assert_eq!(config.get_warehouse_location(), "s3://b/p");

        // Logical warehouse (i.e. Apache Polaris catalog name) with separate storage location.
        let config: RestCatalogConfig = serde_json::from_str(
            r#"{"name":"c","uri":"http://h:8181/api/catalog","warehouse":"polaris_catalog","warehouse_location":"s3://b/p"}"#,
        )
        .unwrap();
        assert_eq!(config.warehouse, "polaris_catalog");
        assert_eq!(config.get_warehouse_location(), "s3://b/p");
    }
}
