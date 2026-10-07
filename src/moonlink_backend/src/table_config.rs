use std::path::Path;

use crate::{Error, Result};
use moonlink::row::IdentityProp;
#[cfg(feature = "catalog-rest")]
use moonlink::IcebergRestCatalogConfig;
use moonlink::MooncakeTableId;
use moonlink::{
    AccessorConfig, DataCompactionConfig, FileIndexMergeConfig, IcebergCatalogConfig,
    IcebergTableConfig, MooncakeTableConfig, MoonlinkTableConfig, StorageConfig, WalConfig,
};
/// Configuration on table creation.
use serde::{Deserialize, Serialize};

/// Mooncake table config.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct MooncakeConfig {
    /// Whether background regular index merge is enabled.
    #[serde(default)]
    pub skip_index_merge: bool,
    /// Whether background regular data compaction is enabled.
    #[serde(default)]
    pub skip_data_compaction: bool,
    /// Whether this is an append-only table (no indexes, no deletes).
    #[serde(default)]
    pub append_only: Option<bool>,
    /// Row identity of the table.
    #[serde(default)]
    pub row_identity: Option<IdentityProp>,
}

impl MooncakeConfig {
    /// Return whether config is valid.
    pub fn is_valid(&self) -> bool {
        if self.append_only.is_none() || self.row_identity.is_none() {
            return false;
        }

        if self.append_only.unwrap() && *self.row_identity.as_ref().unwrap() != IdentityProp::None {
            return false;
        }
        if *self.row_identity.as_ref().unwrap() == IdentityProp::None && !self.append_only.unwrap()
        {
            return false;
        }
        true
    }

    /// Convert to mooncake table config.
    pub(crate) fn take_as_mooncake_table_config(
        self,
        temp_files_dir: String,
    ) -> Result<MooncakeTableConfig> {
        if !self.is_valid() {
            return Err(Error::invalid_config(format!(
                "Invalid config for {:?}",
                &self
            )));
        }

        let index_merge_config = if self.skip_index_merge {
            FileIndexMergeConfig::disabled()
        } else {
            FileIndexMergeConfig::enabled()
        };
        let data_compaction_config = if self.skip_data_compaction {
            DataCompactionConfig::disabled()
        } else {
            DataCompactionConfig::enabled()
        };

        let mut mooncake_table_config = MooncakeTableConfig::new(temp_files_dir);
        mooncake_table_config.file_index_config = index_merge_config;
        mooncake_table_config.data_compaction_config = data_compaction_config;
        mooncake_table_config.append_only = self.append_only.unwrap();
        mooncake_table_config.row_identity = self.row_identity.unwrap();
        Ok(mooncake_table_config)
    }
}

/// Mooncake table configuration specified at creation.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct TableConfig {
    /// Mooncake table configuration.
    #[serde(rename = "mooncake")]
    #[serde(default)]
    pub mooncake_config: MooncakeConfig,

    /// Iceberg storage config.
    #[serde(rename = "iceberg")]
    #[serde(default)]
    pub iceberg_config: Option<AccessorConfig>,

    /// WAL storage config.
    #[serde(rename = "wal")]
    #[serde(default)]
    pub wal_config: Option<AccessorConfig>,

    /// Iceberg REST catalog (i.e. Apache Polaris) to register tables with.
    /// If unassigned (default option), iceberg metadata is managed by a file catalog under the iceberg storage.
    #[cfg(feature = "catalog-rest")]
    #[serde(rename = "rest_catalog")]
    #[serde(default)]
    pub rest_catalog_config: Option<IcebergRestCatalogConfig>,
}

impl TableConfig {
    /// Return whether the config is valid.
    pub fn is_valid(&self) -> bool {
        if !self.mooncake_config.is_valid() {
            return false;
        }
        true
    }

    /// Convert table config from serialized plain json string.
    pub fn from_json_or_default(json: &str, default_table_directory: &str) -> Result<Self> {
        let mut config: TableConfig = serde_json::from_str(json)?;
        if config.iceberg_config.is_none() {
            let storage_config = StorageConfig::FileSystem {
                root_directory: default_table_directory.to_string(),
                // By default disable atomic write option.
                atomic_write_dir: None,
            };
            config.iceberg_config = Some(AccessorConfig::new_with_storage_config(storage_config));
        }
        if config.wal_config.is_none() {
            let storage_config =
                WalConfig::default_storage_config_local(Path::new(default_table_directory));
            config.wal_config = Some(AccessorConfig::new_with_storage_config(storage_config));
        }

        Ok(config)
    }

    /// Get iceberg catalog config, which is the rest catalog if assigned, otherwise a file catalog under the iceberg storage.
    fn get_iceberg_catalog_config(&self) -> IcebergCatalogConfig {
        #[cfg(feature = "catalog-rest")]
        if let Some(rest_catalog_config) = &self.rest_catalog_config {
            return IcebergCatalogConfig::Rest {
                rest_catalog_config: rest_catalog_config.clone(),
            };
        }
        IcebergCatalogConfig::File {
            accessor_config: self.iceberg_config.clone().unwrap(),
        }
    }

    /// Convert to moonlink config.
    pub(crate) fn take_as_moonlink_config(
        self,
        temp_files_dir: String,
        mooncake_table_id: &MooncakeTableId,
    ) -> Result<MoonlinkTableConfig> {
        if !self.is_valid() {
            return Err(Error::invalid_config(format!(
                "Invalid config for {:?}",
                &self
            )));
        }

        let metadata_accessor_config = self.get_iceberg_catalog_config();
        let config = MoonlinkTableConfig {
            mooncake_table_config: self
                .mooncake_config
                .take_as_mooncake_table_config(temp_files_dir)?,
            iceberg_table_config: IcebergTableConfig {
                namespace: vec![mooncake_table_id.database.clone()],
                table_name: mooncake_table_id.table.clone(),
                data_accessor_config: self.iceberg_config.clone().unwrap(),
                metadata_accessor_config,
            },
            wal_table_config: WalConfig::new(
                self.wal_config.unwrap(),
                &mooncake_table_id.to_string(),
            ),
        };
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_table_config_from_empty_json() {
        let actual_table_config =
            TableConfig::from_json_or_default("{}", /*default_table_directory=*/ "/tmp/path")
                .unwrap();
        let expected_table_config = TableConfig {
            mooncake_config: MooncakeConfig {
                skip_index_merge: false,
                skip_data_compaction: false,
                append_only: None,
                row_identity: None,
            },
            iceberg_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp/path".to_string(),
                    atomic_write_dir: None,
                },
            )),
            wal_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp/path".to_string(),
                    atomic_write_dir: None,
                },
            )),
            #[cfg(feature = "catalog-rest")]
            rest_catalog_config: None,
        };
        assert_eq!(actual_table_config, expected_table_config);
    }

    #[test]
    fn test_table_config_from_valid_json() {
        let serialized = r#"
            {
                "mooncake": {
                    "skip_index_merge": true
                },
                "iceberg": {
                    "storage_config": {
                        "fs": {
                            "root_directory": "/tmp"
                        }
                    }
                },
                "wal": {
                    "storage_config": {
                        "fs": {
                            "root_directory": "/tmp/wal"
                        }
                    }
                }
            }
        "#;

        // Deserialize and check.
        let actual_table_config = TableConfig::from_json_or_default(
            serialized,
            /*default_table_directory=*/ "/tmp/path",
        )
        .unwrap();
        let expected_table_config = TableConfig {
            mooncake_config: MooncakeConfig {
                skip_index_merge: true,
                skip_data_compaction: false,
                append_only: None,
                row_identity: None,
            },
            iceberg_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp".to_string(),
                    atomic_write_dir: None,
                },
            )),
            wal_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp/wal".to_string(),
                    atomic_write_dir: None,
                },
            )),
            #[cfg(feature = "catalog-rest")]
            rest_catalog_config: None,
        };
        assert_eq!(expected_table_config, actual_table_config);
    }

    #[test]
    #[cfg(feature = "storage-gcs")]
    fn test_table_config_from_valid_json_with_gcs() {
        let serialized = r#"
            {
                "mooncake": {
                    "skip_index_merge": true
                },
                "iceberg": {
                    "storage_config": {
                        "gcs": {
                            "project": "gcs-proj",
                            "region": "us-west1",
                            "bucket": "moonlink",
                            "access_key_id": "access-key",
                            "secret_access_key": "secret"
                        }
                    }
                },
                "wal": {
                    "storage_config": {
                        "gcs": {
                            "project": "gcs-proj",
                            "region": "us-west1",
                            "bucket": "moonlink-wal",
                            "access_key_id": "access-key-wal",
                            "secret_access_key": "secret-wal"
                        }
                    }
                }
            }
        "#;

        // Deserialize and check.
        let actual_table_config = TableConfig::from_json_or_default(
            serialized,
            /*default_table_directory=*/ "/tmp/path",
        )
        .unwrap();
        let expected_table_config = TableConfig {
            mooncake_config: MooncakeConfig {
                skip_index_merge: true,
                skip_data_compaction: false,
                append_only: None,
                row_identity: None,
            },
            iceberg_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::Gcs {
                    project: "gcs-proj".to_string(),
                    region: "us-west1".to_string(),
                    bucket: "moonlink".to_string(),
                    access_key_id: "access-key".to_string(),
                    secret_access_key: "secret".to_string(),
                    endpoint: None,
                    disable_auth: false,
                    write_option: None,
                },
            )),
            wal_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::Gcs {
                    project: "gcs-proj".to_string(),
                    region: "us-west1".to_string(),
                    bucket: "moonlink-wal".to_string(),
                    access_key_id: "access-key-wal".to_string(),
                    secret_access_key: "secret-wal".to_string(),
                    endpoint: None,
                    disable_auth: false,
                    write_option: None,
                },
            )),
            #[cfg(feature = "catalog-rest")]
            rest_catalog_config: None,
        };
        assert_eq!(expected_table_config, actual_table_config);
    }

    #[test]
    #[cfg(feature = "storage-s3")]
    fn test_table_config_from_valid_json_with_s3() {
        let serialized = r#"
            {
                "mooncake": {
                    "skip_index_merge": true
                },
                "iceberg": {
                    "storage_config": {
                        "s3": {
                            "region": "us-west1",
                            "bucket": "moonlink",
                            "access_key_id": "access-key",
                            "secret_access_key": "secret"
                        }
                    }
                },
                "wal": {
                    "storage_config": {
                        "s3": {
                            "region": "us-west1",
                            "bucket": "moonlink-wal",
                            "access_key_id": "access-key-wal",
                            "secret_access_key": "secret-wal"
                        }
                    }
                }
            }
        "#;

        // Deserialize and check.
        let actual_table_config = TableConfig::from_json_or_default(
            serialized,
            /*default_table_directory=*/ "/tmp/path",
        )
        .unwrap();
        let expected_table_config = TableConfig {
            mooncake_config: MooncakeConfig {
                skip_index_merge: true,
                skip_data_compaction: false,
                append_only: None,
                row_identity: None,
            },
            iceberg_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::S3 {
                    region: "us-west1".to_string(),
                    bucket: "moonlink".to_string(),
                    access_key_id: "access-key".to_string(),
                    secret_access_key: "secret".to_string(),
                    endpoint: None,
                },
            )),
            wal_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::S3 {
                    region: "us-west1".to_string(),
                    bucket: "moonlink-wal".to_string(),
                    access_key_id: "access-key-wal".to_string(),
                    secret_access_key: "secret-wal".to_string(),
                    endpoint: None,
                },
            )),
            #[cfg(feature = "catalog-rest")]
            rest_catalog_config: None,
        };
        assert_eq!(expected_table_config, actual_table_config);
    }

    #[test]
    fn test_table_config_with_append_only() {
        let serialized = r#"
            {
                "mooncake": {
                    "append_only": true,
                    "row_identity": "None",
                    "skip_index_merge": true,
                    "skip_data_compaction": true
                }
            }
        "#;

        // Deserialize and check.
        let actual_table_config = TableConfig::from_json_or_default(
            serialized,
            /*default_table_directory=*/ "/tmp/path",
        )
        .unwrap();
        let expected_table_config = TableConfig {
            mooncake_config: MooncakeConfig {
                skip_index_merge: true,
                skip_data_compaction: true,
                append_only: Some(true),
                row_identity: Some(IdentityProp::None),
            },
            iceberg_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp/path".to_string(),
                    atomic_write_dir: None,
                },
            )),
            wal_config: Some(AccessorConfig::new_with_storage_config(
                moonlink::StorageConfig::FileSystem {
                    root_directory: "/tmp/path".to_string(),
                    atomic_write_dir: None,
                },
            )),
            #[cfg(feature = "catalog-rest")]
            rest_catalog_config: None,
        };
        assert_eq!(expected_table_config, actual_table_config);
    }

    #[test]
    #[cfg(feature = "catalog-rest")]
    fn test_table_config_with_rest_catalog() {
        let serialized = r#"
            {
                "mooncake": {
                    "append_only": true,
                    "row_identity": "None"
                },
                "rest_catalog": {
                    "uri": "http://polaris:8181/api/catalog",
                    "warehouse": "rcp_gold",
                    "warehouse_location": "s3://warehouse/rcp_gold",
                    "props": {
                        "credential": "client-id:client-secret",
                        "scope": "PRINCIPAL_ROLE:ALL"
                    }
                }
            }
        "#;

        let table_config = TableConfig::from_json_or_default(
            serialized,
            /*default_table_directory=*/ "/tmp/path",
        )
        .unwrap();
        let rest_catalog_config = table_config.rest_catalog_config.clone().unwrap();
        assert_eq!(rest_catalog_config.uri, "http://polaris:8181/api/catalog");
        assert_eq!(rest_catalog_config.warehouse, "rcp_gold");
        assert_eq!(
            rest_catalog_config.get_warehouse_location(),
            "s3://warehouse/rcp_gold"
        );

        // Rest catalog is used for iceberg metadata, while data stays on iceberg storage.
        let moonlink_config = table_config
            .take_as_moonlink_config(
                /*temp_files_dir=*/ "/tmp/path".to_string(),
                &MooncakeTableId {
                    database: "db".to_string(),
                    table: "table".to_string(),
                },
            )
            .unwrap();
        let iceberg_table_config = moonlink_config.iceberg_table_config;
        assert_eq!(
            iceberg_table_config.metadata_accessor_config,
            IcebergCatalogConfig::Rest {
                rest_catalog_config
            }
        );
        assert_eq!(
            iceberg_table_config.data_accessor_config,
            AccessorConfig::new_with_storage_config(StorageConfig::FileSystem {
                root_directory: "/tmp/path".to_string(),
                atomic_write_dir: None,
            })
        );
    }
}
