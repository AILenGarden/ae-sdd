use db_operator_native::vault::CredentialVault;

#[test]
fn encrypted_vault_round_trips_without_persisting_plaintext() {
    let directory = tempfile::tempdir().unwrap();
    let vault = CredentialVault::open(directory.path()).unwrap();

    vault
        .store("warehouse", "correct horse battery staple")
        .unwrap();

    let persisted = std::fs::read_dir(directory.path())
        .unwrap()
        .flat_map(|entry| {
            let path = entry.unwrap().path();
            if path.is_dir() {
                std::fs::read_dir(path)
                    .unwrap()
                    .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
                    .collect::<Vec<_>>()
            } else {
                vec![std::fs::read(path).unwrap()]
            }
        })
        .collect::<Vec<_>>();
    assert!(
        persisted.iter().all(|bytes| {
            !String::from_utf8_lossy(bytes).contains("correct horse battery staple")
        })
    );

    let password = vault.read("warehouse").unwrap();
    assert_eq!(password.as_str(), "correct horse battery staple");
    vault.delete("warehouse").unwrap();
    assert!(vault.read("warehouse").is_err());
}

#[test]
fn encrypted_vault_fails_closed_when_master_key_is_missing() {
    let directory = tempfile::tempdir().unwrap();
    let vault = CredentialVault::open(directory.path()).unwrap();
    vault.store("warehouse", "secret").unwrap();
    std::fs::remove_file(directory.path().join("master.key")).unwrap();

    assert!(vault.read("warehouse").is_err());
}
