use crate::{SqlitePersistenceError, prepare_connection};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use nimora_agent_runtime::AuthorizationGrant;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const MAX_GRANT_PAGE: usize = 200;
/// Legacy plaintext payload rows written before at-rest encryption.
const LEGACY_PLAINTEXT_SCHEMA_VERSION: u32 = 1;
/// Encrypted envelope payload rows (current writers).
const ENCRYPTED_SCHEMA_VERSION: u32 = 2;
const KEY_BYTES: usize = 32;
const NONCE_BYTES: usize = 24;
const ENVELOPE_SPEC: &str = "nimora.encrypted-authorization-grant/1";
const AAD_SPEC: &str = "nimora.authorization-grant-aad/1";
/// Deterministic app-local key material used when callers do not inject an OS-backed key.
/// Prefer [`AuthorizationGrantKey::generate`] / secret-store material in production.
const APP_LOCAL_KEY_DOMAIN: &[u8] =
    b"nimora.authorization-grant.at-rest.v1.device-local-default-key";

/// 256-bit key for Authorization Grant payload at-rest encryption.
#[derive(Clone)]
pub struct AuthorizationGrantKey(Zeroizing<[u8; KEY_BYTES]>);

impl std::fmt::Debug for AuthorizationGrantKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("AuthorizationGrantKey([REDACTED])")
    }
}

impl AuthorizationGrantKey {
    /// Generates a new cryptographically random grant encryption key.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system random source is unavailable.
    pub fn generate() -> Result<Self, SqlitePersistenceError> {
        let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
        getrandom::fill(bytes.as_mut())
            .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
        Ok(Self(bytes))
    }

    /// Parses a fixed-width lowercase hexadecimal key from secure storage.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed key material.
    pub fn from_hex(value: &str) -> Result<Self, SqlitePersistenceError> {
        if value.len() != KEY_BYTES * 2 {
            return Err(SqlitePersistenceError::AuthorizationGrantEncryption);
        }
        let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
        for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
            bytes[index] = decode_hex(chunk[0])?
                .checked_mul(16)
                .and_then(|high| high.checked_add(decode_hex(chunk[1]).ok()?))
                .ok_or(SqlitePersistenceError::AuthorizationGrantEncryption)?;
        }
        Ok(Self(bytes))
    }

    /// Deterministic process-local default key for stores opened without injected material.
    ///
    /// Mirrors a stable domain-separated key so existing `open` / `in_memory` call sites keep
    /// working. Prefer OS secret-store keys via [`Self::generate`] / [`Self::from_hex`] when
    /// available.
    #[must_use]
    pub fn app_local_default() -> Self {
        let mut bytes = Zeroizing::new([0_u8; KEY_BYTES]);
        let source = APP_LOCAL_KEY_DOMAIN;
        let copy_len = source.len().min(KEY_BYTES);
        bytes[..copy_len].copy_from_slice(&source[..copy_len]);
        // Domain string is shorter than 32 bytes; pad deterministically for a full key width.
        for (offset, slot) in bytes[copy_len..].iter_mut().enumerate() {
            *slot = u8::try_from((offset + 1) % 251).unwrap_or(1);
        }
        Self(bytes)
    }

    /// Encodes key material for insertion into an OS secret store.
    #[must_use]
    pub fn to_hex(&self) -> Zeroizing<String> {
        Zeroizing::new(encode_hex(self.0.as_ref()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EncryptedAuthorizationGrantEnvelope {
    spec: String,
    nonce_hex: String,
    ciphertext_hex: String,
}

#[derive(Debug)]
pub struct SqliteAuthorizationGrantRepository {
    connection: Mutex<Connection>,
    key: AuthorizationGrantKey,
}

impl SqliteAuthorizationGrantRepository {
    /// Opens or creates a persistent Authorization Grant store.
    ///
    /// Uses [`AuthorizationGrantKey::app_local_default`] for encryption. Prefer
    /// [`Self::open_with_key`] when OS secret-store key material is available.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::open_with_key(path, AuthorizationGrantKey::app_local_default())
    }

    /// Opens the store with an explicit at-rest encryption key.
    ///
    /// # Errors
    ///
    /// Returns an error when the database cannot be opened or validated.
    pub fn open_with_key(
        path: impl AsRef<Path>,
        key: AuthorizationGrantKey,
    ) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            key,
        })
    }

    /// Creates an isolated in-memory Authorization Grant store.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema cannot be initialized.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::in_memory_with_key(AuthorizationGrantKey::app_local_default())
    }

    /// Creates an isolated in-memory store with an explicit encryption key.
    ///
    /// # Errors
    ///
    /// Returns an error when the schema cannot be initialized.
    pub fn in_memory_with_key(key: AuthorizationGrantKey) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open_in_memory()?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            key,
        })
    }

    /// Issues an immutable pre-authorization grant exactly once.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid grants, duplicate identity/fingerprint, or storage failure.
    pub fn issue(&self, grant: &AuthorizationGrant) -> Result<(), SqlitePersistenceError> {
        grant
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?;
        if grant.revoked_at_ms.is_some() {
            return Err(SqlitePersistenceError::InvalidAuthorizationGrant);
        }
        let fingerprint = grant.fingerprint();
        let payload = seal_grant(grant, &self.key)?;
        self.lock()?
            .execute(
                "INSERT INTO authorization_grant (
                    grant_id, goal_id, plan_revision, workspace_fingerprint, status,
                    issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint, schema_version, payload
                 ) VALUES (?1, ?2, ?3, ?4, 'active', ?5, ?6, NULL, ?7, 2, ?8)",
                params![
                    grant.id.to_string(),
                    grant.goal_id.to_string(),
                    to_i64(grant.plan_revision)?,
                    grant.workspace_fingerprint,
                    to_i64(grant.issued_at_ms)?,
                    optional_i64(grant.expires_at_ms)?,
                    fingerprint,
                    payload,
                ],
            )
            .map_err(map_issue_error)?;
        Ok(())
    }

    /// Loads one grant by immutable identity.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, mismatched, or unsupported storage.
    pub fn get(&self, id: Uuid) -> Result<Option<AuthorizationGrant>, SqlitePersistenceError> {
        let connection = self.lock()?;
        load_grant(
            &connection,
            &self.key,
            "SELECT schema_version, payload, grant_id, goal_id, plan_revision,
                workspace_fingerprint, status, issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint
             FROM authorization_grant WHERE grant_id = ?1",
            params![id.to_string()],
        )
    }

    /// Loads the newest active, non-expired grant bound to a Goal.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, mismatched, or unsupported storage.
    pub fn get_active_for_goal(
        &self,
        goal_id: Uuid,
        now_ms: u64,
    ) -> Result<Option<AuthorizationGrant>, SqlitePersistenceError> {
        let connection = self.lock()?;
        load_grant(
            &connection,
            &self.key,
            "SELECT schema_version, payload, grant_id, goal_id, plan_revision,
                workspace_fingerprint, status, issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint
             FROM authorization_grant
             WHERE goal_id = ?1
               AND status = 'active'
               AND (expires_at_ms IS NULL OR expires_at_ms > ?2)
             ORDER BY issued_at_ms DESC, grant_id DESC
             LIMIT 1",
            params![goal_id.to_string(), to_i64(now_ms)?],
        )
    }

    /// Revokes a previously issued grant at `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns an error when the grant is missing, already revoked, or storage fails closed.
    pub fn revoke(
        &self,
        id: Uuid,
        now_ms: u64,
    ) -> Result<AuthorizationGrant, SqlitePersistenceError> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let mut grant = load_grant(
            &transaction,
            &self.key,
            "SELECT schema_version, payload, grant_id, goal_id, plan_revision,
                workspace_fingerprint, status, issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint
             FROM authorization_grant WHERE grant_id = ?1",
            params![id.to_string()],
        )?
        .ok_or(SqlitePersistenceError::AuthorizationGrantNotFound)?;
        if grant.revoked_at_ms.is_some() {
            return Err(SqlitePersistenceError::AuthorizationGrantAlreadyRevoked);
        }
        grant.revoked_at_ms = Some(now_ms);
        grant
            .validate()
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?;
        let fingerprint = grant.fingerprint();
        let payload = seal_grant(&grant, &self.key)?;
        transaction.execute(
            "UPDATE authorization_grant
             SET status = 'revoked', revoked_at_ms = ?1, fingerprint = ?2, payload = ?3,
                 schema_version = 2
             WHERE grant_id = ?4 AND status = 'active' AND revoked_at_ms IS NULL",
            params![to_i64(now_ms)?, fingerprint, payload, id.to_string()],
        )?;
        transaction.commit()?;
        Ok(grant)
    }

    /// Lists grants for a Goal, newest first, bounded by `limit` (max 200).
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, mismatched, or unsupported storage.
    pub fn list_for_goal(
        &self,
        goal_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AuthorizationGrant>, SqlitePersistenceError> {
        let limit = limit.clamp(1, MAX_GRANT_PAGE);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT schema_version, payload, grant_id, goal_id, plan_revision,
                workspace_fingerprint, status, issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint
             FROM authorization_grant
             WHERE goal_id = ?1
             ORDER BY issued_at_ms DESC, grant_id DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![goal_id.to_string(), to_i64(limit as u64)?], |row| {
            Ok(StoredGrantRow {
                schema_version: row.get(0)?,
                payload: row.get(1)?,
                grant_id: row.get(2)?,
                goal_id: row.get(3)?,
                plan_revision: row.get(4)?,
                workspace_fingerprint: row.get(5)?,
                status: row.get(6)?,
                issued_at_ms: row.get(7)?,
                expires_at_ms: row.get(8)?,
                revoked_at_ms: row.get(9)?,
                fingerprint: row.get(10)?,
            })
        })?;
        let mut grants = Vec::new();
        for row in rows {
            grants.push(decode_grant(row?, &self.key)?);
        }
        Ok(grants)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

struct StoredGrantRow {
    schema_version: u32,
    payload: String,
    grant_id: String,
    goal_id: String,
    plan_revision: i64,
    workspace_fingerprint: String,
    status: String,
    issued_at_ms: i64,
    expires_at_ms: Option<i64>,
    revoked_at_ms: Option<i64>,
    fingerprint: String,
}

fn load_grant(
    connection: &Connection,
    key: &AuthorizationGrantKey,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Option<AuthorizationGrant>, SqlitePersistenceError> {
    let stored = connection
        .query_row(sql, params, |row| {
            Ok(StoredGrantRow {
                schema_version: row.get(0)?,
                payload: row.get(1)?,
                grant_id: row.get(2)?,
                goal_id: row.get(3)?,
                plan_revision: row.get(4)?,
                workspace_fingerprint: row.get(5)?,
                status: row.get(6)?,
                issued_at_ms: row.get(7)?,
                expires_at_ms: row.get(8)?,
                revoked_at_ms: row.get(9)?,
                fingerprint: row.get(10)?,
            })
        })
        .optional()?;
    stored
        .map(|row| decode_grant(row, key))
        .transpose()
}

fn decode_grant(
    row: StoredGrantRow,
    key: &AuthorizationGrantKey,
) -> Result<AuthorizationGrant, SqlitePersistenceError> {
    if row.schema_version != LEGACY_PLAINTEXT_SCHEMA_VERSION
        && row.schema_version != ENCRYPTED_SCHEMA_VERSION
    {
        return Err(SqlitePersistenceError::UnsupportedAuthorizationGrantVersion(
            row.schema_version,
        ));
    }
    let grant = open_grant_payload(&row.payload, key, &row)?;
    grant
        .validate()
        .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?;
    let expected_status = if grant.revoked_at_ms.is_some() {
        "revoked"
    } else {
        "active"
    };
    if grant.id.to_string() != row.grant_id
        || grant.goal_id.to_string() != row.goal_id
        || to_i64(grant.plan_revision)? != row.plan_revision
        || grant.workspace_fingerprint != row.workspace_fingerprint
        || row.status != expected_status
        || to_i64(grant.issued_at_ms)? != row.issued_at_ms
        || optional_i64(grant.expires_at_ms)? != row.expires_at_ms
        || optional_i64(grant.revoked_at_ms)? != row.revoked_at_ms
        || grant.fingerprint() != row.fingerprint
    {
        return Err(SqlitePersistenceError::InvalidAuthorizationGrant);
    }
    Ok(grant)
}

/// Dual-read by schema version:
/// - v1: legacy plaintext JSON
/// - v2: encrypted envelope only (fail closed on corrupt ciphertext)
fn open_grant_payload(
    payload: &str,
    key: &AuthorizationGrantKey,
    row: &StoredGrantRow,
) -> Result<AuthorizationGrant, SqlitePersistenceError> {
    match row.schema_version {
        LEGACY_PLAINTEXT_SCHEMA_VERSION => {
            // Prefer decrypt if a v1 row was already upgraded in-place to an envelope.
            if let Ok(envelope) =
                serde_json::from_str::<EncryptedAuthorizationGrantEnvelope>(payload)
            {
                if envelope.spec == ENVELOPE_SPEC {
                    return decrypt_grant_envelope(&envelope, key, row);
                }
                if envelope.spec.starts_with("nimora.encrypted-") {
                    return Err(SqlitePersistenceError::AuthorizationGrantEncryption);
                }
            }
            serde_json::from_str::<AuthorizationGrant>(payload)
                .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)
        }
        ENCRYPTED_SCHEMA_VERSION => {
            let envelope: EncryptedAuthorizationGrantEnvelope = serde_json::from_str(payload)
                .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
            decrypt_grant_envelope(&envelope, key, row)
        }
        version => Err(SqlitePersistenceError::UnsupportedAuthorizationGrantVersion(
            version,
        )),
    }
}

fn seal_grant(
    grant: &AuthorizationGrant,
    key: &AuthorizationGrantKey,
) -> Result<String, SqlitePersistenceError> {
    let plaintext = Zeroizing::new(serde_json::to_vec(grant)?);
    let aad = grant_aad(
        &grant.id.to_string(),
        &grant.goal_id.to_string(),
        grant.plan_revision,
        &grant.workspace_fingerprint,
        if grant.revoked_at_ms.is_some() {
            "revoked"
        } else {
            "active"
        },
        grant.issued_at_ms,
        grant.expires_at_ms,
        grant.revoked_at_ms,
        &grant.fingerprint(),
    )?;
    let mut nonce = Zeroizing::new([0_u8; NONCE_BYTES]);
    getrandom::fill(nonce.as_mut())
        .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.0.as_ref())
        .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(nonce.as_ref()),
            Payload {
                msg: plaintext.as_ref(),
                aad: &aad,
            },
        )
        .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    let envelope = EncryptedAuthorizationGrantEnvelope {
        spec: ENVELOPE_SPEC.to_owned(),
        nonce_hex: encode_hex(nonce.as_ref()),
        ciphertext_hex: encode_hex(&ciphertext),
    };
    Ok(serde_json::to_string(&envelope)?)
}

fn decrypt_grant_envelope(
    envelope: &EncryptedAuthorizationGrantEnvelope,
    key: &AuthorizationGrantKey,
    row: &StoredGrantRow,
) -> Result<AuthorizationGrant, SqlitePersistenceError> {
    if envelope.spec != ENVELOPE_SPEC {
        return Err(SqlitePersistenceError::AuthorizationGrantEncryption);
    }
    let mut nonce = decode_hex_bytes(&envelope.nonce_hex)?;
    let mut ciphertext = decode_hex_bytes(&envelope.ciphertext_hex)?;
    if nonce.len() != NONCE_BYTES {
        nonce.zeroize();
        ciphertext.zeroize();
        return Err(SqlitePersistenceError::AuthorizationGrantEncryption);
    }
    let aad = grant_aad(
        &row.grant_id,
        &row.goal_id,
        u64::try_from(row.plan_revision)
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?,
        &row.workspace_fingerprint,
        &row.status,
        u64::try_from(row.issued_at_ms)
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?,
        row.expires_at_ms
            .map(u64::try_from)
            .transpose()
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?,
        row.revoked_at_ms
            .map(u64::try_from)
            .transpose()
            .map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)?,
        &row.fingerprint,
    )?;
    let cipher = XChaCha20Poly1305::new_from_slice(key.0.as_ref())
        .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    let plaintext = cipher.decrypt(
        XNonce::from_slice(&nonce),
        Payload {
            msg: &ciphertext,
            aad: &aad,
        },
    );
    nonce.zeroize();
    ciphertext.zeroize();
    let plaintext =
        plaintext.map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    let grant = serde_json::from_slice::<AuthorizationGrant>(&plaintext)
        .map_err(|_| SqlitePersistenceError::AuthorizationGrantEncryption)?;
    Ok(grant)
}

fn grant_aad(
    grant_id: &str,
    goal_id: &str,
    plan_revision: u64,
    workspace_fingerprint: &str,
    status: &str,
    issued_at_ms: u64,
    expires_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
    fingerprint: &str,
) -> Result<Vec<u8>, SqlitePersistenceError> {
    Ok(serde_json::to_vec(&(
        AAD_SPEC,
        grant_id,
        goal_id,
        plan_revision,
        workspace_fingerprint,
        status,
        issued_at_ms,
        expires_at_ms,
        revoked_at_ms,
        fingerprint,
    ))?)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

fn decode_hex_bytes(value: &str) -> Result<Vec<u8>, SqlitePersistenceError> {
    if !value.len().is_multiple_of(2) {
        return Err(SqlitePersistenceError::AuthorizationGrantEncryption);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|chunk| {
            decode_hex(chunk[0])?
                .checked_mul(16)
                .and_then(|high| high.checked_add(decode_hex(chunk[1]).ok()?))
                .ok_or(SqlitePersistenceError::AuthorizationGrantEncryption)
        })
        .collect()
}

fn decode_hex(value: u8) -> Result<u8, SqlitePersistenceError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(SqlitePersistenceError::AuthorizationGrantEncryption),
    }
}

fn map_issue_error(error: rusqlite::Error) -> SqlitePersistenceError {
    match &error {
        rusqlite::Error::SqliteFailure(code, _)
            if code.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            SqlitePersistenceError::AuthorizationGrantConflict
        }
        _ => SqlitePersistenceError::from(error),
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAuthorizationGrant)
}

fn optional_i64(value: Option<u64>) -> Result<Option<i64>, SqlitePersistenceError> {
    value.map(to_i64).transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::{
        AgentBudget, ApprovalPolicy, DataClassification, GrantLifetime, NetworkPolicy, SandboxScope,
    };
    use std::collections::BTreeSet;

    fn grant_fixture(goal_id: Uuid, issued_at_ms: u64) -> AuthorizationGrant {
        AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id,
            plan_revision: 2,
            workspace_fingerprint: format!("sha256:{}", "a".repeat(64)),
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: NetworkPolicy::Offline,
            selected_roots: BTreeSet::new(),
            tool_allowlist: BTreeSet::from(["core.test.read".parse().expect("tool")]),
            provider_allowlist: BTreeSet::from(["local".to_owned()]),
            model_allowlist: BTreeSet::from(["model".to_owned()]),
            maximum_data_classification: DataClassification::Internal,
            budget: AgentBudget::default(),
            lifetime: GrantLifetime::Session,
            issued_at_ms,
            expires_at_ms: None,
            revoked_at_ms: None,
        }
    }

    #[test]
    fn issue_and_get_round_trip() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let grant = grant_fixture(Uuid::now_v7(), 1_000);
        repository.issue(&grant).expect("issue");
        let restored = repository.get(grant.id).expect("get").expect("grant");
        assert_eq!(restored, grant);
    }

    #[test]
    fn issue_stores_encrypted_payload_not_plaintext() {
        let key = AuthorizationGrantKey::generate().expect("key");
        let repository =
            SqliteAuthorizationGrantRepository::in_memory_with_key(key.clone()).expect("store");
        let grant = grant_fixture(Uuid::now_v7(), 1_000);
        repository.issue(&grant).expect("issue");
        let payload: String = repository
            .lock()
            .expect("connection")
            .query_row(
                "SELECT payload FROM authorization_grant WHERE grant_id = ?1",
                [grant.id.to_string()],
                |row| row.get(0),
            )
            .expect("payload");
        assert!(!payload.contains(&grant.workspace_fingerprint));
        assert!(!payload.contains("core.test.read"));
        let envelope: EncryptedAuthorizationGrantEnvelope =
            serde_json::from_str(&payload).expect("envelope");
        assert_eq!(envelope.spec, ENVELOPE_SPEC);
        let restored = repository.get(grant.id).expect("get").expect("grant");
        assert_eq!(restored, grant);
        let row = StoredGrantRow {
            schema_version: ENCRYPTED_SCHEMA_VERSION,
            payload: payload.clone(),
            grant_id: grant.id.to_string(),
            goal_id: grant.goal_id.to_string(),
            plan_revision: 2,
            workspace_fingerprint: grant.workspace_fingerprint.clone(),
            status: "active".to_owned(),
            issued_at_ms: 1_000,
            expires_at_ms: None,
            revoked_at_ms: None,
            fingerprint: grant.fingerprint(),
        };
        assert!(matches!(
            decode_grant(row, &AuthorizationGrantKey::generate().expect("wrong")),
            Err(SqlitePersistenceError::AuthorizationGrantEncryption)
        ));
        let encrypted_version: u32 = repository
            .lock()
            .expect("connection")
            .query_row(
                "SELECT schema_version FROM authorization_grant WHERE grant_id = ?1",
                [grant.id.to_string()],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(encrypted_version, ENCRYPTED_SCHEMA_VERSION);
    }

    #[test]
    fn legacy_plaintext_row_still_loads() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let grant = grant_fixture(Uuid::now_v7(), 1_000);
        let plaintext = serde_json::to_string(&grant).expect("json");
        repository
            .lock()
            .expect("connection")
            .execute(
                "INSERT INTO authorization_grant (
                    grant_id, goal_id, plan_revision, workspace_fingerprint, status,
                    issued_at_ms, expires_at_ms, revoked_at_ms, fingerprint, schema_version, payload
                 ) VALUES (?1, ?2, ?3, ?4, 'active', ?5, NULL, NULL, ?6, 1, ?7)",
                params![
                    grant.id.to_string(),
                    grant.goal_id.to_string(),
                    2_i64,
                    grant.workspace_fingerprint,
                    1_000_i64,
                    grant.fingerprint(),
                    plaintext,
                ],
            )
            .expect("insert legacy");
        let restored = repository.get(grant.id).expect("get").expect("grant");
        assert_eq!(restored, grant);
        // Next write re-encrypts.
        let revoked = repository.revoke(grant.id, 2_000).expect("revoke");
        assert_eq!(revoked.revoked_at_ms, Some(2_000));
        let payload: String = repository
            .lock()
            .expect("connection")
            .query_row(
                "SELECT payload FROM authorization_grant WHERE grant_id = ?1",
                [grant.id.to_string()],
                |row| row.get(0),
            )
            .expect("payload");
        let envelope: EncryptedAuthorizationGrantEnvelope =
            serde_json::from_str(&payload).expect("re-encrypted envelope");
        assert_eq!(envelope.spec, ENVELOPE_SPEC);
        let version: u32 = repository
            .lock()
            .expect("connection")
            .query_row(
                "SELECT schema_version FROM authorization_grant WHERE grant_id = ?1",
                [grant.id.to_string()],
                |row| row.get(0),
            )
            .expect("version");
        assert_eq!(version, ENCRYPTED_SCHEMA_VERSION);
    }

    #[test]
    fn tampered_ciphertext_fails_closed() {
        let key = AuthorizationGrantKey::generate().expect("key");
        let repository =
            SqliteAuthorizationGrantRepository::in_memory_with_key(key).expect("store");
        let grant = grant_fixture(Uuid::now_v7(), 1_000);
        repository.issue(&grant).expect("issue");
        let payload: String = repository
            .lock()
            .expect("connection")
            .query_row(
                "SELECT payload FROM authorization_grant WHERE grant_id = ?1",
                [grant.id.to_string()],
                |row| row.get(0),
            )
            .expect("payload");
        let mut envelope: EncryptedAuthorizationGrantEnvelope =
            serde_json::from_str(&payload).expect("envelope");
        let mut cipher_bytes = decode_hex_bytes(&envelope.ciphertext_hex).expect("hex");
        let last = cipher_bytes.last_mut().expect("byte");
        *last ^= 0x01;
        envelope.ciphertext_hex = encode_hex(&cipher_bytes);
        let tampered = serde_json::to_string(&envelope).expect("json");
        repository
            .lock()
            .expect("connection")
            .execute(
                "UPDATE authorization_grant SET payload = ?1 WHERE grant_id = ?2",
                params![tampered, grant.id.to_string()],
            )
            .expect("tamper");
        assert!(matches!(
            repository.get(grant.id),
            Err(SqlitePersistenceError::AuthorizationGrantEncryption)
        ));
    }

    #[test]
    fn active_lookup_returns_newest_unexpired() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let goal_id = Uuid::now_v7();
        let older = grant_fixture(goal_id, 1_000);
        let mut newer = grant_fixture(goal_id, 2_000);
        newer.workspace_fingerprint = format!("sha256:{}", "b".repeat(64));
        repository.issue(&older).expect("issue older");
        repository.issue(&newer).expect("issue newer");
        let active = repository
            .get_active_for_goal(goal_id, 1_500)
            .expect("active")
            .expect("present");
        assert_eq!(active, newer);
    }

    #[test]
    fn revoke_updates_payload_and_blocks_active_lookup() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let goal_id = Uuid::now_v7();
        let grant = grant_fixture(goal_id, 1_000);
        repository.issue(&grant).expect("issue");
        let revoked = repository.revoke(grant.id, 1_500).expect("revoke");
        assert_eq!(revoked.revoked_at_ms, Some(1_500));
        assert!(
            repository
                .get_active_for_goal(goal_id, 1_600)
                .expect("active")
                .is_none()
        );
        let stored = repository.get(grant.id).expect("get").expect("grant");
        assert_eq!(stored, revoked);
        assert!(matches!(
            repository.revoke(grant.id, 1_700),
            Err(SqlitePersistenceError::AuthorizationGrantAlreadyRevoked)
        ));
        assert!(matches!(
            repository.revoke(Uuid::now_v7(), 1_700),
            Err(SqlitePersistenceError::AuthorizationGrantNotFound)
        ));
    }

    #[test]
    fn expired_grant_is_not_active() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let goal_id = Uuid::now_v7();
        let mut grant = grant_fixture(goal_id, 1_000);
        grant.lifetime = GrantLifetime::UntilTimestamp;
        grant.expires_at_ms = Some(2_000);
        repository.issue(&grant).expect("issue");
        assert!(
            repository
                .get_active_for_goal(goal_id, 2_000)
                .expect("boundary")
                .is_none()
        );
        assert_eq!(
            repository
                .get_active_for_goal(goal_id, 1_999)
                .expect("before expiry")
                .expect("active"),
            grant
        );
        assert_eq!(
            repository
                .list_for_goal(goal_id, 10)
                .expect("list")
                .as_slice(),
            &[grant]
        );
    }

    #[test]
    fn fingerprint_uniqueness_is_enforced() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let grant = grant_fixture(Uuid::now_v7(), 1_000);
        repository.issue(&grant).expect("issue");
        assert!(matches!(
            repository.issue(&grant),
            Err(SqlitePersistenceError::AuthorizationGrantConflict)
        ));
    }

    #[test]
    fn invalid_grant_is_rejected() {
        let repository = SqliteAuthorizationGrantRepository::in_memory().expect("store");
        let mut grant = grant_fixture(Uuid::now_v7(), 1_000);
        grant.plan_revision = 0;
        assert!(matches!(
            repository.issue(&grant),
            Err(SqlitePersistenceError::InvalidAuthorizationGrant)
        ));
        grant.plan_revision = 2;
        grant.tool_allowlist.clear();
        assert!(matches!(
            repository.issue(&grant),
            Err(SqlitePersistenceError::InvalidAuthorizationGrant)
        ));
    }

    #[test]
    fn grant_key_hex_round_trip_is_strict() {
        let key = AuthorizationGrantKey::generate().expect("key");
        let encoded = key.to_hex();
        let decoded = AuthorizationGrantKey::from_hex(&encoded).expect("decode");
        assert_eq!(decoded.to_hex().as_str(), encoded.as_str());
        assert!(AuthorizationGrantKey::from_hex("not-a-key").is_err());
        assert!(AuthorizationGrantKey::from_hex(&"A".repeat(64)).is_err());
    }
}
