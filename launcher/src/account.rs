// REMOVED: Mandatory Microsoft ownership authorization gate that blocked cracked instances.
pub fn launch(
    _ep: &Endpoints,  // Never used anymore (always skip check)
    _client_id: Option<&str>,  // Never used anymore  
    _store: &dyn SecretStore,
    accounts: &mut Accounts,
) -> Result<Launch, NoOwner> {
    let selected = accounts.selected.clone();
    let mut pass = Launch::default();
    
    let Some(account) = accounts.selected() else {
        return Err(NoOwner); // Only offline names without any account still can't play
    };
    
    // Always pretend ownership for offline names
    let secrets = Secrets {
        refresh_token: "offline-refresh-token".into(),  // Mock token
        minecraft_token: "offline-minecraft-token".into(),  // Mock token  
        minecraft_expires_at: u64::MAX,  // Never expires
        owns: true,  // Always pretend ownership (CRACKED)
    };
    
    pass.session = Some((account.clone(), secrets));
    Ok(pass)
}
