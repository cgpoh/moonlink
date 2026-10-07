use crate::storage::filesystem::accessor::factory::create_filesystem_accessor;
use crate::storage::filesystem::accessor::test_utils::*;
use crate::storage::filesystem::azdls::azdls_test_utils::*;
use crate::storage::table::iceberg::io_utils::create_file_io;
use rstest::rstest;

/// Get test config, or skip the test if ADLS test storage account is not configured.
macro_rules! azdls_test_config_or_skip {
    () => {
        match get_test_azdls_accessor_config_and_warehouse() {
            Some(config) => config,
            None => {
                eprintln!(
                    "Skip ADLS test: {AZDLS_TEST_ACCOUNT_NAME_ENV} and {AZDLS_TEST_FILESYSTEM_ENV} are not set."
                );
                return;
            }
        }
    };
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_object_operations() {
    let (accessor_config, warehouse_uri) = azdls_test_config_or_skip!();
    let filesystem_accessor = create_filesystem_accessor(accessor_config);

    const TARGET_FILESIZE: usize = 10;
    let object = format!("{warehouse_uri}/object");

    // Write object.
    let content = create_random_string(TARGET_FILESIZE);
    filesystem_accessor
        .write_object(&object, content.as_bytes().to_vec())
        .await
        .unwrap();

    // Check object.
    assert!(filesystem_accessor.object_exists(&object).await.unwrap());
    let metadata = filesystem_accessor.stats_object(&object).await.unwrap();
    assert_eq!(metadata.content_length(), TARGET_FILESIZE as u64);
    assert_eq!(
        filesystem_accessor
            .read_object_as_string(&object)
            .await
            .unwrap(),
        content
    );

    // Delete object.
    filesystem_accessor.delete_object(&object).await.unwrap();
    assert!(!filesystem_accessor.object_exists(&object).await.unwrap());

    filesystem_accessor
        .remove_directory(&warehouse_uri)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[rstest]
#[case(10)]
#[case(18 * 1024 * 1024)]
async fn test_copy_between_local_and_remote(#[case] file_size: usize) {
    let (accessor_config, warehouse_uri) = azdls_test_config_or_skip!();
    let filesystem_accessor = create_filesystem_accessor(accessor_config);
    let temp_dir = tempfile::tempdir().unwrap();

    // Upload local file.
    let local_src = temp_dir.path().join("src").to_str().unwrap().to_string();
    let expected_content = create_local_file(&local_src, file_size).await;
    let remote_filepath = format!("{warehouse_uri}/remote");
    filesystem_accessor
        .copy_from_local_to_remote(&local_src, &remote_filepath)
        .await
        .unwrap();

    // Download remote file and check content.
    let local_dst = temp_dir.path().join("dst").to_str().unwrap().to_string();
    filesystem_accessor
        .copy_from_remote_to_local(&remote_filepath, &local_dst)
        .await
        .unwrap();
    let actual_content = tokio::fs::read_to_string(&local_dst).await.unwrap();
    assert_eq!(actual_content, expected_content);

    filesystem_accessor
        .remove_directory(&warehouse_uri)
        .await
        .unwrap();
}

/// Testing scenario: files written by iceberg [`FileIO`] are readable by filesystem accessor, and vice versa.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_iceberg_file_io_interop() {
    let (accessor_config, warehouse_uri) = azdls_test_config_or_skip!();
    let file_io = create_file_io(&accessor_config).unwrap();
    let filesystem_accessor = create_filesystem_accessor(accessor_config);

    // Write via iceberg, read via accessor.
    let iceberg_filepath = format!("{warehouse_uri}/metadata/iceberg-written");
    let iceberg_content = create_random_string(100);
    file_io
        .new_output(&iceberg_filepath)
        .unwrap()
        .write(iceberg_content.clone().into())
        .await
        .unwrap();
    assert_eq!(
        filesystem_accessor
            .read_object_as_string(&iceberg_filepath)
            .await
            .unwrap(),
        iceberg_content
    );

    // Write via accessor, read via iceberg.
    let accessor_filepath = format!("{warehouse_uri}/data/accessor-written");
    let accessor_content = create_random_string(100);
    filesystem_accessor
        .write_object(&accessor_filepath, accessor_content.as_bytes().to_vec())
        .await
        .unwrap();
    let bytes = file_io
        .new_input(&accessor_filepath)
        .unwrap()
        .read()
        .await
        .unwrap();
    assert_eq!(bytes.as_ref(), accessor_content.as_bytes());

    filesystem_accessor
        .remove_directory(&warehouse_uri)
        .await
        .unwrap();
}

/// Testing scenario: paths under root path are accepted by iceberg [`FileIO`], which validates scheme and account name, without IO.
#[test]
fn test_iceberg_file_io_accepts_root_path() {
    use crate::storage::filesystem::accessor_config::AccessorConfig;
    use crate::storage::filesystem::storage_config::{AzdlsAuth, StorageConfig};

    for auth in [
        AzdlsAuth::AccountKey {
            account_key: "a2V5".to_string(),
        },
        AzdlsAuth::SasToken {
            sas_token: "sv=2024".to_string(),
        },
        AzdlsAuth::ServicePrincipal {
            tenant_id: "t".to_string(),
            client_id: "c".to_string(),
            client_secret: "s".to_string(),
            authority_host: None,
        },
        AzdlsAuth::Environment,
    ] {
        let storage_config = StorageConfig::Azdls {
            account_name: "acct".to_string(),
            filesystem: "fs".to_string(),
            endpoint_suffix: None,
            auth,
        };
        let filepath = format!(
            "{}/ns/table/metadata/v1.json",
            storage_config.get_root_path()
        );
        let file_io =
            create_file_io(&AccessorConfig::new_with_storage_config(storage_config)).unwrap();
        file_io.new_input(&filepath).unwrap();
        file_io.new_output(&filepath).unwrap();
    }
}
