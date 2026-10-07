use crate::storage::mooncake_table::test_utils_commons::{
    POLARIS_CATALOG_TEST_URI, REST_CATALOG_TEST_URI,
};
use crate::storage::table::iceberg::iceberg_table_config::RestCatalogConfig;
use crate::{AccessorConfig, FsRetryConfig, FsTimeoutConfig, IcebergTableConfig, StorageConfig};
use iceberg::spec::{NestedField, PrimitiveType, Schema, Type};
use iceberg::TableCreation;
use rand::{distr::Alphanumeric, Rng};
use std::collections::HashMap;
use tempfile::TempDir;

const DEFAULT_REST_CATALOG_NAME: &str = "test";
const DEFAULT_WAREHOUSE_PATH: &str = "/tmp/moonlink_iceberg";
/// Environment variable to select the rest catalog server under test, either "fixture" (default) or "polaris".
const REST_CATALOG_TEST_SERVER_ENV: &str = "MOONLINK_TEST_REST_CATALOG";
/// Polaris catalog and principal, provisioned by `.devcontainer/polaris-setup.sh`.
const POLARIS_TEST_CATALOG: &str = "moonlink_test";
const POLARIS_TEST_CREDENTIAL: &str = "root:s3cr3t";

pub(crate) fn get_random_string() -> String {
    let rng = rand::rng();
    rng.sample_iter(&Alphanumeric)
        .take(10)
        .map(char::from)
        .collect()
}

pub(crate) fn default_accessor_config() -> AccessorConfig {
    let storage_config = StorageConfig::FileSystem {
        root_directory: DEFAULT_WAREHOUSE_PATH.to_string(),
        atomic_write_dir: None,
    };
    AccessorConfig::new_with_storage_config(storage_config)
}

pub(crate) fn default_rest_catalog_config() -> RestCatalogConfig {
    if std::env::var(REST_CATALOG_TEST_SERVER_ENV).as_deref() == Ok("polaris") {
        return polaris_rest_catalog_config();
    }
    RestCatalogConfig {
        name: format!("{}-{}", DEFAULT_REST_CATALOG_NAME, get_random_string()),
        uri: REST_CATALOG_TEST_URI.to_string(),
        warehouse: DEFAULT_WAREHOUSE_PATH.to_string(),
        warehouse_location: None,
        props: HashMap::new(),
    }
}

/// Apache Polaris takes a logical catalog name as warehouse, so storage location is specified separately.
fn polaris_rest_catalog_config() -> RestCatalogConfig {
    RestCatalogConfig {
        name: format!("{}-{}", DEFAULT_REST_CATALOG_NAME, get_random_string()),
        uri: POLARIS_CATALOG_TEST_URI.to_string(),
        warehouse: POLARIS_TEST_CATALOG.to_string(),
        warehouse_location: Some(DEFAULT_WAREHOUSE_PATH.to_string()),
        props: HashMap::from([
            (
                "credential".to_string(),
                POLARIS_TEST_CREDENTIAL.to_string(),
            ),
            ("scope".to_string(), "PRINCIPAL_ROLE:ALL".to_string()),
        ]),
    }
}

pub(crate) fn get_accessor_config(tmp_dir: &TempDir) -> AccessorConfig {
    let storage_config = StorageConfig::FileSystem {
        root_directory: tmp_dir.path().to_str().unwrap().to_string(),
        atomic_write_dir: None,
    };
    AccessorConfig {
        storage_config,
        retry_config: FsRetryConfig::default(),
        timeout_config: FsTimeoutConfig::default(),
        throttle_config: None,
        chaos_config: None,
    }
}

pub(crate) fn get_rest_iceberg_table_config(tmp_dir: &TempDir) -> IcebergTableConfig {
    IcebergTableConfig {
        namespace: vec![get_random_string()],
        table_name: get_random_string(),
        data_accessor_config: get_accessor_config(tmp_dir),
        metadata_accessor_config: crate::IcebergCatalogConfig::Rest {
            rest_catalog_config: default_rest_catalog_config(),
        },
    }
}

pub(crate) fn default_table_creation(table_name: String) -> TableCreation {
    TableCreation::builder()
        .name(table_name)
        .schema(
            Schema::builder()
                .with_fields(vec![
                    NestedField::optional(1, "foo", Type::Primitive(PrimitiveType::String)).into(),
                    NestedField::required(2, "bar", Type::Primitive(PrimitiveType::Int)).into(),
                    NestedField::optional(3, "baz", Type::Primitive(PrimitiveType::Boolean)).into(),
                ])
                .build()
                .unwrap(),
        )
        .build()
}
