use crate::storage::filesystem::accessor_config::AccessorConfig;
use crate::storage::filesystem::storage_config::{AzdlsAuth, StorageConfig};

/// ADLS Gen2 has no local emulator, so tests run against a real storage account, and are skipped unless both are set.
/// Credentials are loaded via [`AzdlsAuth::Environment`], i.e. AZURE_STORAGE_ACCOUNT_KEY, AZURE_STORAGE_SAS_TOKEN or service principal variables.
pub(crate) const AZDLS_TEST_ACCOUNT_NAME_ENV: &str = "MOONLINK_TEST_AZDLS_ACCOUNT_NAME";
pub(crate) const AZDLS_TEST_FILESYSTEM_ENV: &str = "MOONLINK_TEST_AZDLS_FILESYSTEM";

/// Get ADLS storage config for testing, or [`None`] if test storage account is not configured.
pub(crate) fn get_test_azdls_storage_config() -> Option<StorageConfig> {
    let account_name = std::env::var(AZDLS_TEST_ACCOUNT_NAME_ENV).ok()?;
    let filesystem = std::env::var(AZDLS_TEST_FILESYSTEM_ENV).ok()?;
    Some(StorageConfig::Azdls {
        account_name,
        filesystem,
        endpoint_suffix: None,
        auth: AzdlsAuth::Environment,
    })
}

/// Get ADLS accessor config and a unique warehouse uri for testing, or [`None`] if test storage account is not configured.
pub(crate) fn get_test_azdls_accessor_config_and_warehouse() -> Option<(AccessorConfig, String)> {
    let storage_config = get_test_azdls_storage_config()?;
    let warehouse_uri = format!(
        "{}/moonlink-test-{}",
        storage_config.get_root_path(),
        uuid::Uuid::now_v7()
    );
    Some((
        AccessorConfig::new_with_storage_config(storage_config),
        warehouse_uri,
    ))
}
