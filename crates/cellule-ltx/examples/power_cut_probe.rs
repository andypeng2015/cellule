//! Standalone capture and clean-resume checkpoints for a dedicated fault host.
//! The controller records stdout outside the test device and cuts power at READY.

use std::{io::Write, path::Path, sync::Arc};

use cellule_ltx::{
    CellReplica, CellStorageLayout, Db, Limits, LocalSegment, LtxError, Position, SegmentInfo,
    VerifiedPlan, internal::inspect_ltx, restore_exact,
};
use cellule_store::Store;
use object_store::{memory::InMemory, path::Path as ObjectPath};

fn main() -> cellule_ltx::Result<()> {
    let mut args = std::env::args().skip(1);
    let command = arg(&mut args)?;
    let directory = arg(&mut args)?;
    let directory = Path::new(&directory);
    match command.as_str() {
        "capture-write" => capture_write(directory),
        "capture-verify" => {
            let segment = arg(&mut args)?;
            let txid = number(&arg(&mut args)?)?;
            let checksum = number(&arg(&mut args)?)?;
            let digest = digest(&arg(&mut args)?)?;
            capture_verify(
                directory,
                Path::new(&segment),
                Position { txid, checksum },
                digest,
            )
        }
        "resume-write" => resume_write(directory),
        "resume-verify" => {
            let txid = number(&arg(&mut args)?)?;
            let checksum = number(&arg(&mut args)?)?;
            let digest = digest(&arg(&mut args)?)?;
            resume_verify(directory, Position { txid, checksum }, digest)
        }
        _ => Err(LtxError::InvalidState("unknown power-cut probe command")),
    }
}

fn arg(args: &mut impl Iterator<Item = String>) -> cellule_ltx::Result<String> {
    args.next()
        .ok_or(LtxError::InvalidState("missing power-cut probe argument"))
}

fn number(value: &str) -> cellule_ltx::Result<u64> {
    value
        .parse()
        .map_err(|_| LtxError::InvalidState("invalid power-cut probe number"))
}

fn digest(value: &str) -> cellule_ltx::Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(LtxError::InvalidState("invalid power-cut probe digest"));
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let pair = value
            .get(index * 2..index * 2 + 2)
            .ok_or(LtxError::InvalidState("invalid power-cut probe digest"))?;
        *byte = u8::from_str_radix(pair, 16)
            .map_err(|_| LtxError::InvalidState("invalid power-cut probe digest"))?;
    }
    Ok(bytes)
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn capture_write(directory: &Path) -> cellule_ltx::Result<()> {
    let mut db = Db::open(&directory.join("capture.sqlite"), Limits::default())?;
    db.transaction(|tx| {
        tx.execute_batch("CREATE TABLE witness(v); INSERT INTO witness VALUES(7)")
    })?;
    let cut = db.capture()?;
    let [segment] = cut.segments.as_slice() else {
        return Err(LtxError::InvalidState("probe expected one LTX cut"));
    };
    println!("SEGMENT={}", segment.path().display());
    println!("TXID={}", cut.position.txid);
    println!("CHECKSUM={}", cut.position.checksum);
    println!("DIGEST={}", hex(&segment.info().blake3));
    println!("READY_CAPTURE");
    std::io::stdout().flush()?;
    park_forever()
}

fn capture_verify(
    directory: &Path,
    segment: &Path,
    position: Position,
    digest: [u8; 32],
) -> cellule_ltx::Result<()> {
    let bytes = std::fs::read(segment)?;
    let inspected = inspect_ltx(&bytes)?;
    if inspected.blake3 != digest {
        return Err(LtxError::ChecksumMismatch);
    }
    let info = SegmentInfo {
        min_txid: inspected.min_txid,
        max_txid: inspected.max_txid,
        page_size: inspected.page_size,
        database_pages: inspected.commit,
        pre_checksum: inspected.pre_apply_checksum,
        post_checksum: inspected.post_apply_checksum,
        size_bytes: inspected.size_bytes,
        blake3: inspected.blake3,
    };
    let plan = VerifiedPlan::new(
        &[LocalSegment::new(segment.to_owned(), info)],
        position,
        Limits::default(),
    )?;
    let restored = directory.join("capture-restored.sqlite");
    restore_exact(&plan, &restored)?;
    let connection = cellule_ltx::rusqlite::Connection::open(restored)?;
    let value: i64 = connection.query_row("SELECT v FROM witness", [], |row| row.get(0))?;
    if value != 7 {
        return Err(LtxError::ChecksumMismatch);
    }
    println!("VERIFIED_CAPTURE {} {}", position.txid, position.checksum);
    Ok(())
}

fn resume_write(directory: &Path) -> cellule_ltx::Result<()> {
    let mut db = Db::open(&directory.join("resume.sqlite"), Limits::default())?;
    db.transaction(|tx| {
        tx.execute_batch("CREATE TABLE witness(v); INSERT INTO witness VALUES(7)")
    })?;
    let cut = db.capture()?;
    db.persist_continuation()?;
    db.close()?;
    let digest = blake3::hash(&std::fs::read(directory.join("resume.sqlite"))?);
    println!("TXID={}", cut.position.txid);
    println!("CHECKSUM={}", cut.position.checksum);
    println!("DIGEST={}", hex(digest.as_bytes()));
    println!("READY_RESUME");
    std::io::stdout().flush()?;
    park_forever()
}

fn resume_verify(
    directory: &Path,
    position: Position,
    digest: [u8; 32],
) -> cellule_ltx::Result<()> {
    let source = directory.join("resume.sqlite");
    if blake3::hash(&std::fs::read(&source)?).as_bytes() != &digest {
        return Err(LtxError::ChecksumMismatch);
    }
    let replica = CellReplica::new(
        CellStorageLayout::new(
            Store::new(Arc::new(InMemory::new())),
            ObjectPath::from("power-cut-probe"),
            [1; 16],
        ),
        [2; 32],
        [3; 16],
        Limits::default(),
    )?;
    let mut db = replica.open_resumed(&source, &directory.join("resume-verified.sqlite"))?;
    if db.position() != position {
        return Err(LtxError::ChecksumMismatch);
    }
    let value: i64 = db
        .query_with(|connection| {
            connection.query_row("SELECT v FROM witness", [], |row| row.get(0))
        })
        .map_err(|error| LtxError::Other(Box::new(error)))?;
    if value != 7 {
        return Err(LtxError::ChecksumMismatch);
    }
    db.transaction(|tx| tx.execute_batch("INSERT INTO witness VALUES(8)"))?;
    let next = db.capture()?;
    if Some(next.position.txid) != position.txid.checked_add(1)
        || next.segments.first().map(|cut| cut.info().pre_checksum) != Some(position.checksum)
    {
        return Err(LtxError::ChecksumMismatch);
    }
    db.close()?;
    println!(
        "VERIFIED_RESUME {} {}",
        next.position.txid, next.position.checksum
    );
    Ok(())
}

fn park_forever() -> ! {
    loop {
        std::thread::park();
    }
}
