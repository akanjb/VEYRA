//! Stored settings and the audit log (ADR-032). Neither changes usage data, so writing
//! them does not mark the data as changed.

use rusqlite::{OptionalExtension, params};
use veyra_domain::audit::{AuditAction, AuditActor, AuditEntry, AuditLog};
use veyra_domain::settings::SettingsRepository;
use veyra_domain::source::PortError;
use veyra_domain::time::UtcMillis;

use crate::{Store, StoreError};

const fn actor_name(actor: AuditActor) -> &'static str {
    match actor {
        AuditActor::User => "user",
        AuditActor::System => "system",
    }
}

fn corrupt(what: &str, value: &str) -> StoreError {
    StoreError::Corrupt(format!("audit {what} `{value}`"))
}

impl SettingsRepository for Store {
    fn setting(&self, key: &str) -> Result<Option<String>, PortError> {
        Ok(self.with_reader(|connection| {
            Ok(connection
                .prepare_cached("SELECT value_json FROM settings WHERE key = ?1")?
                .query_row([key], |row| row.get(0))
                .optional()?)
        })?)
    }

    fn setting_or_init(&self, key: &str, initial: &str) -> Result<String, PortError> {
        let mut writer = self.lock_writer()?;
        let transaction = writer.transaction().map_err(StoreError::from)?;
        transaction
            .prepare_cached(
                "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (key) DO NOTHING",
            )
            .and_then(|mut insert| insert.execute(params![key, initial, UtcMillis::now().0]))
            .map_err(StoreError::from)?;
        let value: String = transaction
            .prepare_cached("SELECT value_json FROM settings WHERE key = ?1")
            .and_then(|mut select| select.query_row([key], |row| row.get(0)))
            .map_err(StoreError::from)?;
        transaction.commit().map_err(StoreError::from)?;
        Ok(value)
    }

    fn set_setting(&self, key: &str, value: &str) -> Result<(), PortError> {
        self.lock_writer()?
            .prepare_cached(
                "INSERT INTO settings (key, value_json, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT (key) DO UPDATE SET value_json = excluded.value_json, \
                 updated_at = excluded.updated_at",
            )
            .and_then(|mut upsert| upsert.execute(params![key, value, UtcMillis::now().0]))
            .map_err(StoreError::from)?;
        Ok(())
    }
}

impl AuditLog for Store {
    fn record(&self, entry: &AuditEntry) -> Result<(), PortError> {
        self.lock_writer()?
            .prepare_cached(
                "INSERT INTO audit_log (at, actor, action, detail) VALUES (?1, ?2, ?3, ?4)",
            )
            .and_then(|mut insert| {
                insert.execute(params![
                    entry.at.0,
                    actor_name(entry.actor),
                    entry.action.name(),
                    entry.detail
                ])
            })
            .map_err(StoreError::from)?;
        Ok(())
    }

    fn audit_entries(&self, limit: u32) -> Result<Vec<AuditEntry>, PortError> {
        Ok(self.with_reader(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT at, actor, action, detail FROM audit_log ORDER BY id DESC LIMIT ?1",
            )?;
            let rows = statement.query_map([limit], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;
            rows.map(|row| {
                let (at, actor, action, detail) = row?;
                Ok(AuditEntry {
                    at: UtcMillis(at),
                    actor: match actor.as_str() {
                        "user" => AuditActor::User,
                        "system" => AuditActor::System,
                        _ => return Err(corrupt("actor", &actor)),
                    },
                    action: AuditAction::from_name(&action)
                        .ok_or_else(|| corrupt("action", &action))?,
                    detail,
                })
            })
            .collect()
        })?)
    }
}
