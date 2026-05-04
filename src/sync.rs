use std::path::Path;

use chrono::Local;
use colored::Colorize;
use serde::Deserialize;

use crate::error::{MMBakError, Result};
use crate::reader::MMBakFile;
use crate::sheets;

/// TOML configuration for syncing .mmbak balances to Google Sheets.
#[derive(Debug, Deserialize)]
pub struct SyncConfig {
    /// Google Sheets spreadsheet ID
    pub spreadsheet_id: String,
    /// Path to the service-account JSON key file
    pub service_account_key: String,
    /// Account name mappings
    #[serde(default)]
    pub mappings: Vec<AccountMapping>,
}

/// Maps an .mmbak account name to an Entity_Log entity.
#[derive(Debug, Deserialize, Clone)]
pub struct AccountMapping {
    /// Account name as it appears in the .mmbak file
    pub mmbak_name: String,
    /// The entity_name value in Entity_Log
    pub entity_name: String,
    /// The entity_id value in Entity_Log.
    /// If omitted, an XLOOKUP formula is inserted in column B instead.
    pub entity_id: Option<i64>,
}

/// A single row to append to Entity_Log
#[derive(Debug, Clone)]
pub struct EntityLogRow {
    pub timestamp: String,
    /// `None` means an XLOOKUP formula will be used in column B
    pub entity_id: Option<i64>,
    pub denormation_holding: String,
    pub entity_name: String,
    pub valuation: String,
}

impl EntityLogRow {
    /// Build the XLOOKUP formula for entity_id (column B) when it's not provided.
    fn entity_id_formula(&self) -> String {
        format!(
            r#"=XLOOKUP(INDIRECT("D"&ROW()),Entity!$C$2:$C$1005,Entity!$A$2:$A$1005,"NOT_FOUND")"#
        )
    }

    /// Build the XLOOKUP formula for symbol (column E).
    fn symbol_formula(&self) -> String {
        format!(
            r#"=XLOOKUP(INDIRECT("D"&ROW()),Entity!$C$2:$C$1005,Entity!$E$2:$E$1005,"NOT_FOUND")"#
        )
    }

    fn to_vec(&self) -> Vec<String> {
        let entity_id_cell = match self.entity_id {
            Some(id) => id.to_string(),
            None => self.entity_id_formula(),
        };
        vec![
            self.timestamp.clone(),
            entity_id_cell,
            self.denormation_holding.clone(),
            self.entity_name.clone(),
            self.symbol_formula(),
            self.valuation.clone(),
        ]
    }
}

impl SyncConfig {
    /// Load a SyncConfig from a TOML file path.
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: SyncConfig = toml::from_str(&contents)?;
        Ok(config)
    }
}

/// Build Entity_Log rows from the backup file and config mappings.
///
/// Returns `Ok(rows)` with the rows to append, or an error if any mapped
/// account cannot be found in the backup.
pub fn build_rows(backup: &MMBakFile, config: &SyncConfig) -> Result<Vec<EntityLogRow>> {
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut rows = Vec::new();
    for mapping in &config.mappings {
        let asset = backup
            .active_assets()
            .find(|a| a.display_name() == mapping.mmbak_name)
            .ok_or_else(|| {
                MMBakError::EntryNotFound(format!(
                    "Account '{}' not found in .mmbak file",
                    mapping.mmbak_name
                ))
            })?;

        let balance = asset.calculated_balance;
        let balance_str = format!("{:.2}", balance);

        rows.push(EntityLogRow {
            timestamp: now.clone(),
            entity_id: mapping.entity_id,
            denormation_holding: balance_str.clone(),
            entity_name: mapping.entity_name.clone(),
            valuation: balance_str,
        });
    }

    Ok(rows)
}

/// Print a preview of the rows that would be appended.
pub fn preview_rows(rows: &[EntityLogRow]) {
    println!("{}", "Preview: rows to append to Entity_Log".bold().underline());
    println!(
        "{:20} {:8} {:>15} {:30} {:8} {:>15}",
        "Timestamp", "EntityID", "Holding", "Entity Name", "Symbol", "Valuation"
    );
    println!("{}", "─".repeat(110));
    for row in rows {
        let entity_display = match row.entity_id {
            Some(id) => id.to_string(),
            None => "<XLOOKUP>".to_string(),
        };
        println!(
            "{:20} {:8} {:>15} {:30} {:8} {:>15}",
            row.timestamp,
            entity_display,
            row.denormation_holding,
            row.entity_name,
            "<XLOOKUP>",
            row.valuation
        );
    }
    println!("\nTotal rows to append: {}", rows.len());
}

/// Main sync flow: authenticate, build rows, append to Google Sheet.
pub async fn sync_balances(
    backup: &MMBakFile,
    config: &SyncConfig,
    dry_run: bool,
) -> Result<()> {
    let rows = build_rows(backup, config)?;

    if rows.is_empty() {
        println!("No accounts mapped. Nothing to sync.");
        return Ok(());
    }

    if dry_run {
        preview_rows(&rows);
        println!("\n{} (dry-run, no changes made)", "Done".green().bold());
        return Ok(());
    }

    // Convert rows to string vectors
    let values: Vec<Vec<String>> = rows.iter().map(|r| r.to_vec()).collect();

    // Append to sheet
    sheets::append_values(
        Path::new(&config.service_account_key),
        &config.spreadsheet_id,
        "Entity_Log",
        values,
    )
    .await?;

    println!(
        "{} Appended {} row(s) to Entity_Log in spreadsheet {}",
        "✓".green().bold(),
        rows.len(),
        config.spreadsheet_id
    );

    Ok(())
}
