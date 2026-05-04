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
    /// The entity_id value in Entity_Log
    pub entity_id: i64,
    /// The symbol (e.g. "SGD", "BTC")
    pub symbol: String,
}

/// A single row to append to Entity_Log
#[derive(Debug, Clone)]
pub struct EntityLogRow {
    pub timestamp: String,
    pub entity_id: i64,
    pub denormation_holding: String,
    pub entity_name: String,
    pub symbol: String,
    pub valuation: String,
}

impl EntityLogRow {
    fn to_vec(&self) -> Vec<String> {
        vec![
            self.timestamp.clone(),
            self.entity_id.to_string(),
            self.denormation_holding.clone(),
            self.entity_name.clone(),
            self.symbol.clone(),
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
            symbol: mapping.symbol.clone(),
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
        println!(
            "{:20} {:8} {:>15} {:30} {:8} {:>15}",
            row.timestamp,
            row.entity_id,
            row.denormation_holding,
            row.entity_name,
            row.symbol,
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
