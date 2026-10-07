#!/usr/bin/env sh
# Create the Apache Polaris catalog used by rest catalog tests (`MOONLINK_TEST_REST_CATALOG=polaris`).
# Idempotent: an already existing catalog is left untouched.
set -eu

POLARIS_URI="${POLARIS_URI:-http://polaris.local:8181}"
POLARIS_CLIENT_ID="${POLARIS_CLIENT_ID:-root}"
POLARIS_CLIENT_SECRET="${POLARIS_CLIENT_SECRET:-s3cr3t}"
POLARIS_CATALOG="${POLARIS_CATALOG:-moonlink_test}"
POLARIS_WAREHOUSE_LOCATION="${POLARIS_WAREHOUSE_LOCATION:-file:///tmp/moonlink_iceberg}"

# Wait until Polaris is able to issue tokens.
token=""
for _ in $(seq 1 30); do
  token=$(curl -sf "$POLARIS_URI/api/catalog/v1/oauth/tokens" \
    -d grant_type=client_credentials \
    -d client_id="$POLARIS_CLIENT_ID" \
    -d client_secret="$POLARIS_CLIENT_SECRET" \
    -d scope=PRINCIPAL_ROLE:ALL |
    sed -n 's/.*"access_token" *: *"\([^"]*\)".*/\1/p') || true
  [ -n "$token" ] && break
  echo "Waiting for Polaris at $POLARIS_URI..."
  sleep 2
done
if [ -z "$token" ]; then
  echo "ERROR: Polaris failed to start." >&2
  exit 1
fi

status=$(curl -s -o /dev/null -w '%{http_code}' \
  -X POST "$POLARIS_URI/api/management/v1/catalogs" \
  -H "Authorization: Bearer $token" \
  -H 'Content-Type: application/json' \
  -d "{
    \"catalog\": {
      \"name\": \"$POLARIS_CATALOG\",
      \"type\": \"INTERNAL\",
      \"properties\": { \"default-base-location\": \"$POLARIS_WAREHOUSE_LOCATION\" },
      \"storageConfigInfo\": {
        \"storageType\": \"FILE\",
        \"allowedLocations\": [\"$POLARIS_WAREHOUSE_LOCATION\"]
      }
    }
  }")
case "$status" in
  201) echo "Created Polaris catalog $POLARIS_CATALOG." ;;
  409) echo "Polaris catalog $POLARIS_CATALOG already exists." ;;
  *) echo "ERROR: failed to create Polaris catalog, HTTP $status." >&2; exit 1 ;;
esac
