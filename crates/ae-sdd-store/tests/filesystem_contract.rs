use std::path::Path;

use ae_sdd_store::{
    CrossProcessLockPort, DurableFileSystem, InMemoryFileSystem, StdCrossProcessLock,
    StdDurableFileSystem, StoreError,
};

#[test]
fn in_memory_filesystem_implements_durable_io_and_exclusive_locking() {
    let filesystem = InMemoryFileSystem::default();
    let root = Path::new("workspace");
    let first = root.join("a.json");
    let second = root.join("b.json");
    let nested = root.join("nested").join("ignored.json");

    assert_eq!(
        filesystem.read(&first).expect("missing read succeeds"),
        None
    );
    filesystem.insert(&second, b"second".to_vec());
    assert_eq!(filesystem.snapshot(&second), Some(b"second".to_vec()));
    filesystem
        .write_atomic_durable(&first, b"first")
        .expect("atomic write succeeds");
    filesystem
        .write_atomic_durable(&nested, b"nested")
        .expect("nested write succeeds");
    filesystem
        .create_dir_all(&root.join("nested"))
        .expect("directory creation succeeds");
    filesystem
        .sync_directory(root)
        .expect("directory sync succeeds");

    assert_eq!(
        filesystem.read(&first).expect("written file reads"),
        Some(b"first".to_vec())
    );
    assert_eq!(
        filesystem.list_files(root).expect("listing succeeds"),
        vec![first.clone(), second.clone()]
    );

    let guard = filesystem
        .lock_exclusive(&root.join("state.lock"))
        .expect("first exclusive lock succeeds");
    assert!(matches!(
        filesystem.lock_exclusive(&root.join("state.lock")),
        Err(StoreError::LeaseConflict)
    ));
    drop(guard);
    let reacquired = filesystem
        .lock_exclusive(&root.join("state.lock"))
        .expect("dropping the guard releases the lock");
    drop(reacquired);

    let nonblocking = filesystem
        .try_lock_exclusive(&root.join("state.lock"))
        .expect("nonblocking lock attempt succeeds")
        .expect("unheld in-memory lock is acquired");
    assert!(
        filesystem
            .try_lock_exclusive(&root.join("state.lock"))
            .expect("contended nonblocking lock is a normal outcome")
            .is_none()
    );
    drop(nonblocking);
    assert!(
        filesystem
            .try_lock_exclusive(&root.join("state.lock"))
            .expect("released in-memory lock can be retried")
            .is_some()
    );
}

#[test]
fn standard_filesystem_round_trips_sorted_files_and_maps_io_errors() {
    let temp = tempfile::tempdir().expect("temporary directory is created");
    let filesystem = StdDurableFileSystem;
    let data_dir = temp.path().join("data");
    let first = data_dir.join("a.json");
    let second = data_dir.join("b.json");

    assert!(
        filesystem
            .list_files(&temp.path().join("missing"))
            .expect("missing directories list as empty")
            .is_empty()
    );
    filesystem
        .create_dir_all(&data_dir)
        .expect("directory creation succeeds");
    filesystem
        .write_atomic_durable(&second, b"second")
        .expect("second file writes");
    filesystem
        .write_atomic_durable(&first, b"first")
        .expect("first file writes");
    filesystem
        .create_dir_all(&data_dir.join("nested"))
        .expect("nested directory is created");

    assert_eq!(
        filesystem.list_files(&data_dir).expect("listing succeeds"),
        vec![first.clone(), second]
    );
    assert_eq!(
        filesystem.read(&first).expect("file reads"),
        Some(b"first".to_vec())
    );
    assert!(matches!(
        filesystem.read(&data_dir),
        Err(StoreError::Io { path, .. }) if path == data_dir
    ));
    assert!(matches!(
        filesystem.sync_directory(&temp.path().join("missing")),
        Err(StoreError::Io { .. })
    ));
    assert!(matches!(
        filesystem.list_files(&first),
        Err(StoreError::Io { path, .. }) if path == first
    ));
    assert!(matches!(
        filesystem.create_dir_all(&first),
        Err(StoreError::Io { path, .. }) if path == first
    ));
    assert!(matches!(
        filesystem.write_atomic_durable(&data_dir, b"not-a-directory"),
        Err(StoreError::Io { path, .. }) if path == data_dir
    ));
    assert!(matches!(
        filesystem.remove_file_durable(&data_dir),
        Err(StoreError::Io { path, .. }) if path == data_dir
    ));
}

#[test]
fn standard_nonblocking_lock_reports_contention_and_recovers_after_release() {
    let temp = tempfile::tempdir().expect("temporary directory is created");
    let lock = StdCrossProcessLock;
    let path = temp.path().join("locks/state.lock");

    let first = lock
        .try_lock_exclusive(&path)
        .expect("first nonblocking lock attempt succeeds")
        .expect("unheld operating-system lock is acquired");
    assert!(
        lock.try_lock_exclusive(&path)
            .expect("contention is reported without an I/O failure")
            .is_none()
    );
    drop(first);
    assert!(
        lock.try_lock_exclusive(&path)
            .expect("released operating-system lock can be retried")
            .is_some()
    );

    assert!(matches!(
        lock.try_lock_exclusive(temp.path()),
        Err(StoreError::Io { path: failed, .. }) if failed == temp.path()
    ));
}
