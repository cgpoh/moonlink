# Deploy moonlink on Azure (AKS + ADLS Gen2)

Moonlink runs on AKS and writes Iceberg tables (and optionally WAL) to Azure Data Lake Storage Gen2.

## 1. Create storage

ADLS Gen2 requires a storage account with **hierarchical namespace** enabled.

```bash
RG=moonlink-rg
LOCATION=westeurope
ACCOUNT=moonlinkdata          # globally unique, lowercase
az storage account create -g $RG -n $ACCOUNT -l $LOCATION \
  --sku Standard_LRS --kind StorageV2 --hns true

# One filesystem for Iceberg tables, one for WAL.
az storage fs create --account-name $ACCOUNT -n moonlink-iceberg --auth-mode login
az storage fs create --account-name $ACCOUNT -n moonlink-wal --auth-mode login
```

## 2. Pick an identity

Whichever identity moonlink uses needs the **Storage Blob Data Contributor** role on the storage account.

**Option A: AKS managed identity (recommended, no secret).** Moonlink gets a token from the node's instance metadata endpoint, as the kubelet's user-assigned identity:

```bash
AKS=moonlink-aks
KUBELET_CLIENT_ID=$(az aks show -g $RG -n $AKS --query identityProfile.kubeletidentity.clientId -o tsv)
KUBELET_OBJECT_ID=$(az aks show -g $RG -n $AKS --query identityProfile.kubeletidentity.objectId -o tsv)
az role assignment create --assignee-object-id $KUBELET_OBJECT_ID --assignee-principal-type ServicePrincipal \
  --role "Storage Blob Data Contributor" \
  --scope $(az storage account show -g $RG -n $ACCOUNT --query id -o tsv)

kubectl create secret generic moonlink-azure-credentials \
  --from-literal=AZURE_CLIENT_ID=$KUBELET_CLIENT_ID
```

Every pod on the node pool can use this identity. If that's too broad, use option B.

> AKS **Workload Identity** (`AZURE_FEDERATED_TOKEN_FILE`) is not supported: opendal's ADLS Gen2 service has no option for federated tokens.

**Option B: service principal.**

```bash
SP=$(az ad sp create-for-rbac -n moonlink-adls --role "Storage Blob Data Contributor" \
  --scopes $(az storage account show -g $RG -n $ACCOUNT --query id -o tsv))

kubectl create secret generic moonlink-azure-credentials \
  --from-literal=AZURE_TENANT_ID=$(echo $SP | jq -r .tenant) \
  --from-literal=AZURE_CLIENT_ID=$(echo $SP | jq -r .appId) \
  --from-literal=AZURE_CLIENT_SECRET=$(echo $SP | jq -r .password)
```

`AZURE_STORAGE_ACCOUNT_KEY` or `AZURE_STORAGE_SAS_TOKEN` in the same secret also work. They take precedence over the service principal.

## 3. Build and deploy

The default image doesn't include ADLS support, so build with the `storage-azdls` feature:

```bash
ACR=myregistry
IMAGE=$ACR.azurecr.io/moonlink:azdls
docker build -f Dockerfile.amd64 --platform linux/amd64 \
  --build-arg CARGO_FEATURES=moonlink_service/storage-azdls -t $IMAGE .
az acr login -n $ACR && docker push $IMAGE

IMAGE=$IMAGE envsubst < deploy/azure/prod/deployment/moonlink_deployment.yaml | kubectl apply -f -
kubectl apply -f deploy/azure/prod/service/moonlink_service.yaml
```

The cluster needs pull access to the registry (`az aks update -g $RG -n $AKS --attach-acr $ACR`).

## 4. Create a table on ADLS

Storage is configured per table:

```bash
MOONLINK=http://$(kubectl get svc moonlink-service -o jsonpath='{.status.loadBalancer.ingress[0].ip}'):3030

curl -X POST $MOONLINK/tables/users \
  -H "Content-Type: application/json" \
  -d '{
    "database": "my_database",
    "table": "users",
    "schema": [
      {"name": "id", "data_type": "int32", "nullable": false},
      {"name": "name", "data_type": "string", "nullable": false}
    ],
    "table_config": {
      "mooncake": {"append_only": true},
      "iceberg": {
        "storage_config": {
          "azdls": {
            "account_name": "moonlinkdata",
            "filesystem": "moonlink-iceberg",
            "auth": {"type": "environment"}
          }
        }
      },
      "wal": {
        "storage_config": {
          "azdls": {
            "account_name": "moonlinkdata",
            "filesystem": "moonlink-wal",
            "auth": {"type": "environment"}
          }
        }
      }
    }
  }'
```

Tables land at `abfss://moonlink-iceberg@moonlinkdata.dfs.core.windows.net/my_database/users`, readable by any Iceberg engine (Spark, Trino, DuckDB, ...).

### Why `"auth": {"type": "environment"}`

The table config, **including any credentials in it**, is saved as plain JSON in moonlink's metadata store. With `environment`, only `{"type": "environment"}` is saved, and credentials stay in the Kubernetes secret, so you can rotate them without recreating tables.

Explicit methods also work, but put the secret in the metadata store:

```json
"auth": {"type": "account_key", "account_key": "..."}
"auth": {"type": "sas_token", "sas_token": "..."}
"auth": {"type": "service_principal", "tenant_id": "...", "client_id": "...", "client_secret": "..."}
```

For sovereign clouds, add `"endpoint_suffix"`, e.g. `"core.chinacloudapi.cn"`, and set `AZURE_AUTHORITY_HOST` accordingly.

## Caveats

- The metadata store (SQLite) and local cache live in `/tmp/moonlink`, an `emptyDir`, so they're lost when the pod is rescheduled. Data on ADLS survives, but moonlink won't know about its tables after a restart. Back `/tmp/moonlink` with a persistent volume (e.g. Azure Disk) if that matters.
- Delta Lake tables don't support ADLS yet; only Iceberg does.
