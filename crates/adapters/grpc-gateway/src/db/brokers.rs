//! Broker database operations - Plain text storage

use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::db::schema::SCHEMA;

/// Full broker record with all fields including secrets
#[derive(Debug, Clone)]
pub struct BrokerRecord {
    pub id: String,
    pub name: String,
    pub broker_type: String,
    pub api_key: String,
    pub api_secret: String,
    pub user_id: String,
    pub password: String,
    pub totp_secret: Option<String>,
    pub access_token: Option<String>,
    pub token_expiry: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Public broker info with masked secrets for listing
#[derive(Debug, Clone)]
pub struct BrokerInfo {
    pub id: String,
    pub name: String,
    pub broker_type: String,
    pub has_api_key: bool,
    pub has_api_secret: bool,
    pub has_credentials: bool,
    pub has_totp: bool,
    pub has_token: bool,
    pub token_expiry: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Partial update for broker records
#[derive(Debug, Clone, Default)]
pub struct BrokerUpdate {
    pub name: Option<String>,
    pub broker_type: Option<String>,
    pub api_key: Option<String>,
    pub api_secret: Option<String>,
    pub user_id: Option<String>,
    pub password: Option<String>,
    pub totp_secret: Option<Option<String>>,
    pub access_token: Option<Option<String>>,
    pub token_expiry: Option<Option<i64>>,
}

/// Broker database manager - plain SQLite storage
pub struct BrokerDb {
    conn: Mutex<Connection>,
}

impl BrokerDb {
    /// Create new BrokerDb, opening or creating database at the specified path
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create database directory")?;
            }
        }

        let conn = Connection::open(path).context("Failed to open database")?;

        // Initialize schema
        conn.execute_batch(SCHEMA)
            .context("Failed to initialize database schema")?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Save a broker record (INSERT OR REPLACE)
    pub fn save(&self, record: &BrokerRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO brokers (
                id, name, broker_type, api_key, api_secret, user_id, password,
                totp_secret, access_token, token_expiry, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                COALESCE((SELECT created_at FROM brokers WHERE id = ?1), ?11), ?12)",
            params![
                record.id,
                record.name,
                record.broker_type,
                record.api_key,
                record.api_secret,
                record.user_id,
                record.password,
                record.totp_secret,
                record.access_token,
                record.token_expiry,
                now, // created_at (used only if new)
                now, // updated_at
            ],
        )?;
        Ok(())
    }

    /// Get a broker by ID (full record with secrets)
    pub fn get(&self, id: &str) -> Result<Option<BrokerRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, broker_type, api_key, api_secret, user_id, password,
             totp_secret, access_token, token_expiry, created_at, updated_at
             FROM brokers WHERE id = ?1",
        )?;

        let record = stmt
            .query_row(params![id], |row| {
                Ok(BrokerRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    broker_type: row.get(2)?,
                    api_key: row.get(3)?,
                    api_secret: row.get(4)?,
                    user_id: row.get(5)?,
                    password: row.get(6)?,
                    totp_secret: row.get(7)?,
                    access_token: row.get(8)?,
                    token_expiry: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })
            .optional()?;

        Ok(record)
    }

    /// Get broker info by ID (masked, no secrets)
    pub fn get_info(&self, id: &str) -> Result<Option<BrokerInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, broker_type,
             api_key IS NOT NULL AND api_key != '' as has_api_key,
             api_secret IS NOT NULL AND api_secret != '' as has_api_secret,
             (user_id IS NOT NULL AND user_id != '') OR (password IS NOT NULL AND password != '') as has_credentials,
             totp_secret IS NOT NULL AND totp_secret != '' as has_totp,
             access_token IS NOT NULL AND access_token != '' as has_token,
             token_expiry, created_at, updated_at
             FROM brokers WHERE id = ?1",
        )?;

        let info = stmt
            .query_row(params![id], |row| {
                Ok(BrokerInfo {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    broker_type: row.get(2)?,
                    has_api_key: row.get(3)?,
                    has_api_secret: row.get(4)?,
                    has_credentials: row.get(5)?,
                    has_totp: row.get(6)?,
                    has_token: row.get(7)?,
                    token_expiry: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })
            .optional()?;

        Ok(info)
    }

    /// List all brokers (masked info, no secrets)
    pub fn list(&self) -> Result<Vec<BrokerInfo>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, broker_type,
             api_key IS NOT NULL AND api_key != '' as has_api_key,
             api_secret IS NOT NULL AND api_secret != '' as has_api_secret,
             (user_id IS NOT NULL AND user_id != '') OR (password IS NOT NULL AND password != '') as has_credentials,
             totp_secret IS NOT NULL AND totp_secret != '' as has_totp,
             access_token IS NOT NULL AND access_token != '' as has_token,
             token_expiry, created_at, updated_at
             FROM brokers",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BrokerInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                broker_type: row.get(2)?,
                has_api_key: row.get(3)?,
                has_api_secret: row.get(4)?,
                has_credentials: row.get(5)?,
                has_totp: row.get(6)?,
                has_token: row.get(7)?,
                token_expiry: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .context("Failed to list brokers")
    }

    /// Delete a broker by ID
    pub fn delete(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM brokers WHERE id = ?1", params![id])?;
        Ok(rows > 0)
    }

    /// Update access token for a broker
    pub fn update_token(&self, id: &str, access_token: &str, expiry: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "UPDATE brokers SET access_token = ?2, token_expiry = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, access_token, expiry, now],
        )?;
        Ok(())
    }

    /// Partial update for a broker
    pub fn update(&self, id: &str, updates: BrokerUpdate) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        // Build dynamic UPDATE query
        let mut set_clauses = Vec::new();
        let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(ref name) = updates.name {
            set_clauses.push("name = ?");
            values.push(Box::new(name.clone()));
        }
        if let Some(ref broker_type) = updates.broker_type {
            set_clauses.push("broker_type = ?");
            values.push(Box::new(broker_type.clone()));
        }
        if let Some(ref api_key) = updates.api_key {
            set_clauses.push("api_key = ?");
            values.push(Box::new(api_key.clone()));
        }
        if let Some(ref api_secret) = updates.api_secret {
            set_clauses.push("api_secret = ?");
            values.push(Box::new(api_secret.clone()));
        }
        if let Some(ref user_id) = updates.user_id {
            set_clauses.push("user_id = ?");
            values.push(Box::new(user_id.clone()));
        }
        if let Some(ref password) = updates.password {
            set_clauses.push("password = ?");
            values.push(Box::new(password.clone()));
        }
        if let Some(ref totp_secret) = updates.totp_secret {
            set_clauses.push("totp_secret = ?");
            values.push(Box::new(totp_secret.clone()));
        }
        if let Some(ref access_token) = updates.access_token {
            set_clauses.push("access_token = ?");
            values.push(Box::new(access_token.clone()));
        }
        if let Some(ref token_expiry) = updates.token_expiry {
            set_clauses.push("token_expiry = ?");
            values.push(Box::new(*token_expiry));
        }

        if set_clauses.is_empty() {
            return Ok(false);
        }

        set_clauses.push("updated_at = ?");
        values.push(Box::new(now));

        let sql = format!(
            "UPDATE brokers SET {} WHERE id = ?",
            set_clauses.join(", ")
        );
        values.push(Box::new(id.to_string()));

        let params: Vec<&dyn rusqlite::ToSql> = values.iter().map(|v| v.as_ref()).collect();
        let rows = conn.execute(&sql, params.as_slice())?;
        Ok(rows > 0)
    }

    // ============= AI Config Methods =============

    /// Save an AI config value
    pub fn save_ai_config(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT OR REPLACE INTO ai_config (key, value, updated_at) VALUES (?1, ?2, ?3)",
            params![key, value, now],
        )?;
        tracing::info!(
            "Saved AI config: {} = {}...",
            key,
            &value[..value.len().min(10)]
        );
        Ok(())
    }

    /// Get an AI config value
    pub fn get_ai_config(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Option<String> = conn
            .query_row(
                "SELECT value FROM ai_config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result)
    }

    /// Get all AI config values
    pub fn get_all_ai_config(&self) -> Result<HashMap<String, String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM ai_config")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut config = HashMap::new();
        for row in rows {
            let (key, value) = row?;
            config.insert(key, value);
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_db() -> (BrokerDb, NamedTempFile) {
        let file = NamedTempFile::new().unwrap();
        let db = BrokerDb::new(file.path()).unwrap();
        (db, file)
    }

    #[test]
    fn test_save_and_get_broker() {
        let (db, _file) = create_test_db();

        let record = BrokerRecord {
            id: "test-broker-1".to_string(),
            name: "Test Broker".to_string(),
            broker_type: "zerodha".to_string(),
            api_key: "api-key-123".to_string(),
            api_secret: "api-secret-456".to_string(),
            user_id: "user-789".to_string(),
            password: "password-abc".to_string(),
            totp_secret: Some("totp-secret".to_string()),
            access_token: Some("access-token-xyz".to_string()),
            token_expiry: Some(1234567890),
            created_at: 0,
            updated_at: 0,
        };

        db.save(&record).unwrap();

        let retrieved = db.get("test-broker-1").unwrap().unwrap();
        assert_eq!(retrieved.id, "test-broker-1");
        assert_eq!(retrieved.name, "Test Broker");
        assert_eq!(retrieved.api_key, "api-key-123");
        assert_eq!(retrieved.api_secret, "api-secret-456");
        assert_eq!(retrieved.totp_secret, Some("totp-secret".to_string()));
    }

    #[test]
    fn test_get_info_masks_secrets() {
        let (db, _file) = create_test_db();

        let record = BrokerRecord {
            id: "test-broker-2".to_string(),
            name: "Test Broker 2".to_string(),
            broker_type: "zerodha".to_string(),
            api_key: "api-key".to_string(),
            api_secret: "api-secret".to_string(),
            user_id: "user-id".to_string(),
            password: "password".to_string(),
            totp_secret: Some("totp".to_string()),
            access_token: None,
            token_expiry: None,
            created_at: 0,
            updated_at: 0,
        };

        db.save(&record).unwrap();

        let info = db.get_info("test-broker-2").unwrap().unwrap();
        assert_eq!(info.id, "test-broker-2");
        assert!(info.has_api_key);
        assert!(info.has_api_secret);
        assert!(info.has_credentials);
        assert!(info.has_totp);
        assert!(!info.has_token);
    }

    #[test]
    fn test_list_brokers() {
        let (db, _file) = create_test_db();

        for i in 1..=3 {
            let record = BrokerRecord {
                id: format!("broker-{}", i),
                name: format!("Broker {}", i),
                broker_type: "zerodha".to_string(),
                api_key: "key".to_string(),
                api_secret: "secret".to_string(),
                user_id: "user".to_string(),
                password: "pass".to_string(),
                totp_secret: None,
                access_token: None,
                token_expiry: None,
                created_at: 0,
                updated_at: 0,
            };
            db.save(&record).unwrap();
        }

        let brokers = db.list().unwrap();
        assert_eq!(brokers.len(), 3);
    }

    #[test]
    fn test_delete_broker() {
        let (db, _file) = create_test_db();

        let record = BrokerRecord {
            id: "to-delete".to_string(),
            name: "Delete Me".to_string(),
            broker_type: "zerodha".to_string(),
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            user_id: "user".to_string(),
            password: "pass".to_string(),
            totp_secret: None,
            access_token: None,
            token_expiry: None,
            created_at: 0,
            updated_at: 0,
        };

        db.save(&record).unwrap();
        assert!(db.get("to-delete").unwrap().is_some());

        let deleted = db.delete("to-delete").unwrap();
        assert!(deleted);
        assert!(db.get("to-delete").unwrap().is_none());
    }

    #[test]
    fn test_update_token() {
        let (db, _file) = create_test_db();

        let record = BrokerRecord {
            id: "token-test".to_string(),
            name: "Token Test".to_string(),
            broker_type: "zerodha".to_string(),
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            user_id: "user".to_string(),
            password: "pass".to_string(),
            totp_secret: None,
            access_token: None,
            token_expiry: None,
            created_at: 0,
            updated_at: 0,
        };

        db.save(&record).unwrap();
        db.update_token("token-test", "new-token", 9999999999).unwrap();

        let updated = db.get("token-test").unwrap().unwrap();
        assert_eq!(updated.access_token, Some("new-token".to_string()));
        assert_eq!(updated.token_expiry, Some(9999999999));
    }

    #[test]
    fn test_partial_update() {
        let (db, _file) = create_test_db();

        let record = BrokerRecord {
            id: "partial-update".to_string(),
            name: "Original Name".to_string(),
            broker_type: "zerodha".to_string(),
            api_key: "key".to_string(),
            api_secret: "secret".to_string(),
            user_id: "user".to_string(),
            password: "pass".to_string(),
            totp_secret: None,
            access_token: None,
            token_expiry: None,
            created_at: 0,
            updated_at: 0,
        };

        db.save(&record).unwrap();

        let updates = BrokerUpdate {
            name: Some("Updated Name".to_string()),
            api_key: Some("new-key".to_string()),
            ..Default::default()
        };

        let updated = db.update("partial-update", updates).unwrap();
        assert!(updated);

        let retrieved = db.get("partial-update").unwrap().unwrap();
        assert_eq!(retrieved.name, "Updated Name");
        assert_eq!(retrieved.api_key, "new-key");
        assert_eq!(retrieved.api_secret, "secret"); // Unchanged
    }

    #[test]
    fn test_ai_config() {
        let (db, _file) = create_test_db();

        db.save_ai_config("test_key", "test_value").unwrap();

        let value = db.get_ai_config("test_key").unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        let all = db.get_all_ai_config().unwrap();
        assert_eq!(all.get("test_key"), Some(&"test_value".to_string()));
    }
}
