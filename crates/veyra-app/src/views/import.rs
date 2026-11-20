//! Importing a chat app's data export the user chooses (ADR-040).

use veyra_domain::audit::{AuditAction, AuditActor, AuditEntry};
use veyra_domain::import::ImportError;
use veyra_domain::time::UtcMillis;

use crate::DataServices;
use crate::dto::ExportImported;
use crate::error::ApiError;
use crate::views::internal;

/// File types the dialog offers: the archive as the provider sends it, or the
/// conversations document from inside it.
const EXTENSIONS: [&str; 2] = ["zip", "json"];

/// Asks for an export, reads it and records the import in the audit log.
pub fn import(data: &DataServices) -> Result<ExportImported, ApiError> {
    let picked = data
        .picker
        .pick("Data export", &EXTENSIONS)
        .map_err(|message| ApiError::SourceUnavailable { message })?;
    let Some(mut file) = picked else {
        return Ok(ExportImported {
            imported: false,
            app: None,
            conversations: 0,
            skipped: 0,
            replies: 0,
            added: 0,
            updated: 0,
            unchanged: 0,
        });
    };
    let summary = data.importer.import(&mut *file).map_err(|e| match e {
        ImportError::NotRecognized | ImportError::Unreadable(_) => ApiError::Validation {
            message: e.to_string(),
        },
        ImportError::Storage(_) => internal(&e),
    })?;
    let counts = summary.counts;
    // The detail names the app, never the file.
    data.audit
        .record(&AuditEntry {
            at: UtcMillis::now(),
            actor: AuditActor::User,
            action: AuditAction::Import,
            detail: format!(
                "{} replies read from a {} export: {} added, {} updated, {} already stored",
                counts.replies,
                summary.app_name,
                summary.inserted,
                summary.updated,
                summary.unchanged
            ),
        })
        .map_err(|e| internal(&e))?;
    Ok(ExportImported {
        imported: true,
        app: Some(summary.app_name.to_owned()),
        conversations: counts.conversations,
        skipped: counts.skipped,
        replies: counts.replies,
        added: summary.inserted,
        updated: summary.updated,
        unchanged: summary.unchanged,
    })
}
