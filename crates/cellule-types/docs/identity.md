# Storage identities

`StorageProviderKind` names the physical provider family. `BucketIdentity`
normalizes host and container names at construction so equivalent URLs can
share a cache identity.

| Kind | Meaning | Cloud aliases |
| --- | --- | --- |
| `S3` | S3 or compatible storage | `aws`, `s3` |
| `Gcs` | Google Cloud Storage | `gcp`, `gcs`, `gs`, `google` |
| `Azure` | Azure Blob Storage | `azure`, `az`, `abs` |
| `Local` | Explicit local or test storage | Never returned by `parse_cloud_alias`. |

```rust
use cellule_types::storage::{BucketIdentity, StorageProviderKind};

let a = BucketIdentity::new(StorageProviderKind::S3, "Bucket/", "Objects/");
let b = BucketIdentity::new(StorageProviderKind::S3, "bucket", "objects");
assert_eq!(a, b);
assert_ne!(a, BucketIdentity::local_unset());
```

`StorageScope` carries repository and global prefixes plus source and scope
identity. It limits views; it is not an owner lease or an authorization check.
The application validates scope and credentials before constructing a
store. See [cellule-store](../../cellule-store/docs/README.md) for transport.
