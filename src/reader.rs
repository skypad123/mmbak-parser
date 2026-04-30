use std::path::Path;
use rusqlite::Connection;

use crate::error::{MMBakError, Result};
use crate::types::*;

/// Reader for MoneyManager SQLite backup files
pub struct MMBakReader {
    conn: Connection,
    file_size: u64,
}

impl MMBakReader {
    /// Open a MoneyManager backup file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file_size = std::fs::metadata(path)?.len();
        let conn = Connection::open(path)?;
        
        Ok(MMBakReader { conn, file_size })
    }

    /// Parse the complete database
    pub fn parse(self) -> Result<MMBakFile> {
        let transactions = self.read_transactions()?;
        let mut assets = self.read_assets()?;
        let categories = self.read_categories()?;
        let currencies = self.read_currencies()?;
        
        // Calculate balances for each asset based on transactions
        self.calculate_asset_balances(&mut assets, &transactions);
        
        // Calculate statistics
        let stats = self.calculate_stats(&transactions, &assets, &categories, &currencies)?;
        
        Ok(MMBakFile {
            transactions,
            assets,
            categories,
            currencies,
            stats,
            file_size: self.file_size,
        })
    }

    /// Read all transactions from ZINOUTCOME table
    fn read_transactions(&self) -> Result<Vec<Transaction>> {
        let mut stmt = self.conn.prepare(
            "SELECT ZUID, ZDO_TYPE, ZAMOUNT, ZTXDATESTR, ZCATEGORYUID, ZCATEGORY_NAME, 
                    ZASSETUID, ZASSET_NAME, ZTOASSETUID, ZCURRENCYUID, ZMEMO, ZPAID, ZISDEL
             FROM ZINOUTCOME
             ORDER BY ZDATE DESC"
        )?;

        let transactions = stmt.query_map([], |row| {
            let uid: String = row.get(0)?;
            let do_type: Option<String> = row.get(1)?;
            let transaction_type = do_type
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(TransactionType::from_i64)
                .unwrap_or(TransactionType::Expense);
            let amount: f64 = row.get(2)?;
            let date: Option<String> = row.get(3)?;
            let category_uid: Option<String> = row.get(4)?;
            let category_name: Option<String> = row.get(5)?;
            let asset_uid: Option<String> = row.get(6)?;
            let asset_name: Option<String> = row.get(7)?;
            let to_asset_uid: Option<String> = row.get(8)?;
            let currency_uid: Option<String> = row.get(9)?;
            let memo: Option<String> = row.get(10)?;
            let payee: Option<String> = row.get(11)?;
            let is_deleted: i64 = row.get(12).unwrap_or(0);

            Ok(Transaction {
                uid,
                transaction_type,
                amount,
                date: date.unwrap_or_default(),
                category_uid,
                category_name,
                asset_uid,
                asset_name,
                to_asset_uid,
                currency_uid,
                memo,
                payee,
                is_deleted: is_deleted != 0,
            })
        })?;

        transactions
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MMBakError::SqliteError(e))
    }

    /// Read all assets from ZASSET table
    fn read_assets(&self) -> Result<Vec<Asset>> {
        let mut stmt = self.conn.prepare(
            "SELECT ZUID, ZNICNAME, ZTYPE, ZLEFTMONEY, ZCURRENCYUID, ZGROUPUID, ZMEMO, ZISDEL
             FROM ZASSET
             ORDER BY ZORDER"
        )?;

        let assets = stmt.query_map([], |row| {
            let uid: String = row.get(0)?;
            let nicname: Option<String> = row.get(1)?;
            let asset_type: i64 = row.get(2).unwrap_or(0);
            let left_money: i64 = row.get(3).unwrap_or(0);
            let currency_uid: Option<String> = row.get(4)?;
            let group_uid: Option<String> = row.get(5)?;
            let memo: Option<String> = row.get(6)?;
            let is_deleted: i64 = row.get(7).unwrap_or(0);

            Ok(Asset {
                uid: uid.clone(),
                name: nicname.clone().unwrap_or_else(|| uid.clone()),
                nicname,
                asset_type,
                left_money,
                calculated_balance: 0.0,
                currency_uid,
                group_uid,
                memo,
                is_deleted: is_deleted != 0,
            })
        })?;

        assets
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MMBakError::SqliteError(e))
    }

    /// Calculate asset balances from transaction history
    fn calculate_asset_balances(&self, assets: &mut [Asset], transactions: &[Transaction]) {
        use std::collections::HashMap;
        
        // Create a map of asset UID to balance
        let mut balances: HashMap<String, f64> = HashMap::new();
        
        for tx in transactions {
            if tx.is_deleted {
                continue;
            }
            
            match tx.transaction_type {
                TransactionType::Income => {
                    // Income: Add to asset balance
                    if let Some(asset_uid) = &tx.asset_uid {
                        *balances.entry(asset_uid.clone()).or_insert(0.0) += tx.amount;
                    }
                }
                TransactionType::Expense => {
                    // Expense: Subtract from asset balance
                    if let Some(asset_uid) = &tx.asset_uid {
                        *balances.entry(asset_uid.clone()).or_insert(0.0) -= tx.amount;
                    }
                }
                TransactionType::Transfer => {
                    // Transfer: Subtract from source, add to destination
                    if let Some(from_asset_uid) = &tx.asset_uid {
                        *balances.entry(from_asset_uid.clone()).or_insert(0.0) -= tx.amount;
                    }
                    if let Some(to_asset_uid) = &tx.to_asset_uid {
                        *balances.entry(to_asset_uid.clone()).or_insert(0.0) += tx.amount;
                    }
                }
            }
        }
        
        // Apply calculated balances to assets
        for asset in assets.iter_mut() {
            if let Some(balance) = balances.get(&asset.uid) {
                asset.calculated_balance = *balance;
            }
        }
    }

    /// Read all categories from ZCATEGORY table
    fn read_categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT ZUID, ZNAME, ZPUID, ZDOTYPE, ZSTATUS, ZISDEL
             FROM ZCATEGORY
             ORDER BY ZORDER"
        )?;

        let categories = stmt.query_map([], |row| {
            let uid: String = row.get(0)?;
            let name: String = row.get(1)?;
            let parent_uid: Option<String> = row.get(2)?;
            let do_type: i64 = row.get(3).unwrap_or(0);
            let status: i64 = row.get(4).unwrap_or(0);
            let is_deleted: i64 = row.get(5).unwrap_or(0);

            Ok(Category {
                uid,
                name,
                parent_uid,
                do_type,
                status,
                is_deleted: is_deleted != 0,
            })
        })?;

        categories
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MMBakError::SqliteError(e))
    }

    /// Read all currencies from ZCURRENCY table
    fn read_currencies(&self) -> Result<Vec<Currency>> {
        let mut stmt = self.conn.prepare(
            "SELECT ZUID, ZISO, ZSYMBOL, ZRATE, ZISMAINCURRENCY, ZISSHOW
             FROM ZCURRENCY
             ORDER BY ZORDERSEQ"
        )?;

        let currencies = stmt.query_map([], |row| {
            let uid: String = row.get(0)?;
            let iso: String = row.get(1)?;
            let symbol: Option<String> = row.get(2)?;
            let rate: f64 = row.get(3).unwrap_or(1.0);
            let is_main: i64 = row.get(4).unwrap_or(0);
            let is_show: i64 = row.get(5).unwrap_or(1);

            Ok(Currency {
                uid,
                iso,
                symbol,
                rate,
                is_main_currency: is_main != 0,
                is_show: is_show != 0,
            })
        })?;

        currencies
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| MMBakError::SqliteError(e))
    }

    /// Calculate database statistics
    fn calculate_stats(
        &self,
        transactions: &[Transaction],
        assets: &[Asset],
        categories: &[Category],
        currencies: &[Currency],
    ) -> Result<DatabaseStats> {
        let active_transactions: Vec<_> = transactions
            .iter()
            .filter(|t| !t.is_deleted)
            .collect();

        let active_assets: Vec<_> = assets
            .iter()
            .filter(|a| !a.is_deleted)
            .collect();

        let mut total_income = 0.0;
        let mut total_expense = 0.0;

        for tx in &active_transactions {
            match tx.transaction_type {
                TransactionType::Income => total_income += tx.amount,
                TransactionType::Expense => total_expense += tx.amount,
                TransactionType::Transfer => {}, // Transfers don't affect total income/expense
            }
        }

        Ok(DatabaseStats {
            total_transactions: transactions.len(),
            total_assets: assets.len(),
            total_categories: categories.len(),
            total_currencies: currencies.len(),
            active_transactions: active_transactions.len(),
            active_assets: active_assets.len(),
            total_income,
            total_expense,
        })
    }
}

/// Represents a fully parsed MoneyManager backup file
#[derive(Debug)]
pub struct MMBakFile {
    transactions: Vec<Transaction>,
    assets: Vec<Asset>,
    categories: Vec<Category>,
    currencies: Vec<Currency>,
    stats: DatabaseStats,
    file_size: u64,
}

impl MMBakFile {
    /// Open and parse a complete backup file
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let reader = MMBakReader::open(path)?;
        reader.parse()
    }

    /// Get all transactions
    pub fn transactions(&self) -> &[Transaction] {
        &self.transactions
    }

    /// Get active (non-deleted) transactions
    pub fn active_transactions(&self) -> impl Iterator<Item = &Transaction> {
        self.transactions.iter().filter(|t| !t.is_deleted)
    }

    /// Get all assets
    pub fn assets(&self) -> &[Asset] {
        &self.assets
    }

    /// Get active (non-deleted) assets
    pub fn active_assets(&self) -> impl Iterator<Item = &Asset> {
        self.assets.iter().filter(|a| !a.is_deleted)
    }

    /// Get all categories
    pub fn categories(&self) -> &[Category] {
        &self.categories
    }

    /// Get active (non-deleted) categories
    pub fn active_categories(&self) -> impl Iterator<Item = &Category> {
        self.categories.iter().filter(|c| !c.is_deleted)
    }

    /// Get all currencies
    pub fn currencies(&self) -> &[Currency] {
        &self.currencies
    }

    /// Get statistics
    pub fn stats(&self) -> &DatabaseStats {
        &self.stats
    }

    /// Get file size
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Find an asset by UID
    pub fn find_asset(&self, uid: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.uid == uid)
    }

    /// Find a category by UID
    pub fn find_category(&self, uid: &str) -> Option<&Category> {
        self.categories.iter().find(|c| c.uid == uid)
    }

    /// Find a currency by UID
    pub fn find_currency(&self, uid: &str) -> Option<&Currency> {
        self.currencies.iter().find(|c| c.uid == uid)
    }
}
