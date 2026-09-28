use super::*;

#[cfg(feature = "replica")]
fn committed_database(path: &Path, limits: Limits) -> Db {
    let mut db = Db::open(path, limits).unwrap();
    db.transaction(|tx| tx.execute_batch("CREATE TABLE payload(value INTEGER)"))
        .unwrap();
    db.transaction(|tx| tx.execute("INSERT INTO payload VALUES (7)", []))
        .unwrap();
    db.capture().unwrap();
    db
}

#[cfg(feature = "replica")]
fn resumed_payload(db: &mut Db) -> i64 {
    db.query_with(|connection| {
        connection.query_row("SELECT count(*) FROM payload", [], |row| row.get(0))
    })
    .unwrap()
}

#[cfg(feature = "replica")]
#[test]
fn a_recorded_continuation_continues_the_chain_after_a_move() {
    let temp = tempfile::TempDir::new().unwrap();
    let limits = Limits::default();
    let source = temp.path().join("session.sqlite");
    let db = committed_database(&source, limits);
    assert!(!db.has_pending_capture());
    db.persist_continuation().unwrap();
    let position = db.position();
    db.close().unwrap();

    // The capture session directory fences the original path, so a resumed
    // activation always installs the file somewhere nothing has claimed.
    let host = crate::Host::default();
    let destination = temp.path().join("warm.sqlite");
    crate::resume::move_resumed(&source, &destination, &host).unwrap();
    let mut resumed = Db::open_resumed_with_host(&destination, limits, host).unwrap();
    assert_eq!(resumed.position(), position);
    assert_eq!(resumed_payload(&mut resumed), 1);

    resumed
        .transaction(|tx| tx.execute("INSERT INTO payload VALUES (8)", []))
        .unwrap();
    let next = resumed.capture().unwrap();
    assert_eq!(next.position.txid, position.txid + 1);
    assert_eq!(resumed_payload(&mut resumed), 2);
    resumed.close().unwrap();
}

#[cfg(feature = "replica")]
#[test]
fn a_resume_refuses_a_continuation_that_does_not_match_the_file() {
    let temp = tempfile::TempDir::new().unwrap();
    let limits = Limits::default();
    let source = temp.path().join("mismatched.sqlite");
    let db = committed_database(&source, limits);
    db.persist_continuation().unwrap();
    db.close().unwrap();

    let host = crate::Host::default();
    let corrupt = temp.path().join("corrupt.sqlite");
    crate::resume::move_resumed(&source, &corrupt, &host).unwrap();
    let continuation = crate::resume::continuation_path(&corrupt);
    let mut bytes = std::fs::read(&continuation).unwrap();
    let recorded: u32 = bytes[32..36].try_into().map(u32::from_be_bytes).unwrap();
    bytes[32..36].copy_from_slice(&(recorded + 1).to_be_bytes());
    std::fs::write(&continuation, bytes).unwrap();
    assert!(Db::open_resumed_with_host(&corrupt, limits, host.clone()).is_err());

    // A file that no longer holds the recorded image is refused the same way.
    let short = temp.path().join("short.sqlite");
    crate::resume::discard_resumed(&corrupt, &host).unwrap();
    let source = temp.path().join("short-source.sqlite");
    let db = committed_database(&source, limits);
    db.persist_continuation().unwrap();
    db.close().unwrap();
    crate::resume::move_resumed(&source, &short, &host).unwrap();
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&short)
        .unwrap();
    file.set_len(file.metadata().unwrap().len() - 4096).unwrap();
    drop(file);
    assert!(Db::open_resumed_with_host(&short, limits, host).is_err());
}

#[cfg(feature = "replica")]
#[test]
fn a_resume_refuses_same_length_database_or_sidecar_corruption() {
    use std::io::{Read, Seek, SeekFrom, Write};

    for corrupt_sidecar in [false, true] {
        let temp = tempfile::TempDir::new().unwrap();
        let limits = Limits::default();
        let source = temp.path().join("source.sqlite");
        let db = committed_database(&source, limits);
        db.persist_continuation().unwrap();
        db.close().unwrap();

        let host = crate::Host::default();
        let destination = temp.path().join("corrupt.sqlite");
        crate::resume::move_resumed(&source, &destination, &host).unwrap();
        let corrupt = if corrupt_sidecar {
            crate::resume::checksum_path(&destination)
        } else {
            destination.clone()
        };
        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&corrupt)
            .unwrap();
        file.seek(SeekFrom::End(-1)).unwrap();
        let mut byte = [0];
        file.read_exact(&mut byte).unwrap();
        file.seek(SeekFrom::End(-1)).unwrap();
        file.write_all(&[byte[0] ^ 1]).unwrap();
        drop(file);

        assert!(
            matches!(
                Db::open_resumed_with_host(&destination, limits, host),
                Err(crate::LtxError::ChecksumMismatch)
            ),
            "corrupt_sidecar={corrupt_sidecar}"
        );
    }
}

#[cfg(feature = "replica")]
#[test]
fn a_resume_refuses_a_database_that_is_not_checkpointed() {
    let temp = tempfile::TempDir::new().unwrap();
    let limits = Limits::default();
    let source = temp.path().join("dirty.sqlite");
    let db = committed_database(&source, limits);
    db.persist_continuation().unwrap();
    db.close().unwrap();

    // A database whose WAL still holds frames may sit behind the continuation,
    // so a resumed open must fall back to the authoritative root instead.
    std::fs::write(crate::resume::wal_path(&source), [0u8; 32]).unwrap();
    let host = crate::Host::default();
    let destination = temp.path().join("refused.sqlite");
    assert!(crate::resume::move_resumed(&source, &destination, &host).is_err());
    assert!(!destination.exists());
}

#[cfg(feature = "replica")]
#[test]
fn a_dense_checksum_copy_refuses_a_base_that_no_longer_folds_to_it() {
    let temp = tempfile::TempDir::new().unwrap();
    let limits = Limits::default();
    let source = temp.path().join("folding.sqlite");
    let db = committed_database(&source, limits);
    db.persist_continuation().unwrap();
    db.close().unwrap();

    let host = crate::Host::default();
    let destination = temp.path().join("reopened.sqlite");
    crate::resume::move_resumed(&source, &destination, &host).unwrap();
    let resumed = Db::open_resumed_with_host(&destination, limits, host).unwrap();
    let checksums = crate::resume::checksum_path(&destination);
    let mut bytes = std::fs::read(&checksums).unwrap();
    let wrong = u64::from_be_bytes(bytes[0..8].try_into().unwrap()) ^ 1;
    bytes[0..8].copy_from_slice(&wrong.to_be_bytes());
    std::fs::write(&checksums, bytes).unwrap();
    assert!(resumed.persist_continuation().is_err());
}
