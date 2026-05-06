use crate::error::Result;
use crate::reader::MMBakFile;
use crate::types::{Asset, Transaction, TransactionType};
use ratatui::crossterm::event::{self, Event, KeyEventKind};
use ratatui::DefaultTerminal;
use std::collections::HashMap;
use std::time::Duration;

pub mod events;
pub mod ui;

use events::Action;
use ui::render;

/// A row in the account list — either a group header or an actual account.
#[derive(Clone)]
pub enum AccountRow {
    GroupHeader { name: String },
    Account { asset: Asset },
}

impl AccountRow {
    pub fn is_header(&self) -> bool {
        matches!(self, AccountRow::GroupHeader { .. })
    }

    pub fn display_name(&self) -> String {
        match self {
            AccountRow::GroupHeader { name } => name.clone(),
            AccountRow::Account { asset } => asset.display_name().to_string(),
        }
    }

    pub fn balance(&self) -> Option<f64> {
        match self {
            AccountRow::GroupHeader { .. } => None,
            AccountRow::Account { asset } => Some(asset.calculated_balance),
        }
    }

    pub fn asset_uid(&self) -> Option<String> {
        match self {
            AccountRow::GroupHeader { .. } => None,
            AccountRow::Account { asset } => Some(asset.uid.clone()),
        }
    }
}

/// Which panel is currently focused.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActivePanel {
    Accounts,
    Transactions,
}

/// A row in the transactions panel — either a date divider or an actual transaction.
#[derive(Clone)]
pub enum TxRow {
    DateHeader { date: String },
    Tx { tx: Transaction },
}

impl TxRow {
    pub fn is_header(&self) -> bool {
        matches!(self, TxRow::DateHeader { .. })
    }
}

/// Full application state for the TUI.
pub struct App {
    pub backup: MMBakFile,

    // ── Accounts panel ────────────────────────────────────────
    pub account_rows: Vec<AccountRow>,
    pub account_cursor: usize,
    pub selected_asset_uid: Option<String>,

    // ── Transactions panel ───────────────────────────────────
    pub tx_rows: Vec<TxRow>,
    pub tx_cursor: usize,
    pub tx_offset: usize,

    // ── Focus ────────────────────────────────────────────────
    pub active_panel: ActivePanel,

    // ── Exit flag ────────────────────────────────────────────
    pub should_quit: bool,
}

impl App {
    pub fn new(backup: MMBakFile) -> Self {
        let account_rows = build_account_rows(&backup);
        let tx_rows = build_tx_rows(collect_tx(backup.active_transactions()));

        App {
            account_rows,
            account_cursor: 0,
            selected_asset_uid: None,
            tx_rows,
            tx_cursor: 0,
            tx_offset: 0,
            active_panel: ActivePanel::Accounts,
            should_quit: false,
            backup,
        }
    }

    /// Run the main event/draw loop until the user quits.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.should_quit {
            terminal.draw(|frame| render(frame, self))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        if let Some(action) = events::map_key(key.code) {
                            self.apply(action);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn apply(&mut self, action: Action) {
        use Action::*;
        match action {
            Up => self.move_up(),
            Down => self.move_down(),
            SwitchPanel => self.switch_panel(),
            Select => self.select_account(),
            Deselect => self.deselect_account(),
            Quit => self.should_quit = true,
        }
    }

    fn move_up(&mut self) {
        match self.active_panel {
            ActivePanel::Accounts => {
                if self.account_cursor > 0 {
                    self.account_cursor -= 1;
                    // Skip group headers
                    while self.account_cursor > 0
                        && self.account_rows[self.account_cursor].is_header()
                    {
                        self.account_cursor -= 1;
                    }
                }
            }
            ActivePanel::Transactions => {
                if self.tx_cursor > 0 {
                    self.tx_cursor -= 1;
                    while self.tx_cursor > 0 && self.tx_rows[self.tx_cursor].is_header() {
                        self.tx_cursor -= 1;
                    }
                }
            }
        }
    }

    fn move_down(&mut self) {
        match self.active_panel {
            ActivePanel::Accounts => {
                if self.account_cursor + 1 < self.account_rows.len() {
                    self.account_cursor += 1;
                    // Skip group headers
                    while self.account_cursor < self.account_rows.len()
                        && self.account_rows[self.account_cursor].is_header()
                    {
                        self.account_cursor += 1;
                    }
                }
            }
            ActivePanel::Transactions => {
                if self.tx_cursor + 1 < self.tx_rows.len() {
                    self.tx_cursor += 1;
                    while self.tx_cursor < self.tx_rows.len()
                        && self.tx_rows[self.tx_cursor].is_header()
                    {
                        self.tx_cursor += 1;
                    }
                }
            }
        }
    }

    fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            ActivePanel::Accounts => ActivePanel::Transactions,
            ActivePanel::Transactions => ActivePanel::Accounts,
        };
    }

    fn select_account(&mut self) {
        if self.active_panel != ActivePanel::Accounts {
            return;
        }
        if let Some(uid) = self.account_rows[self.account_cursor].asset_uid() {
            self.selected_asset_uid = Some(uid.clone());
            self.tx_rows =
                build_tx_rows(collect_tx(self.backup.active_transactions().filter(|t| {
                    t.asset_uid.as_deref() == Some(&uid) || t.to_asset_uid.as_deref() == Some(&uid)
                })));
            self.tx_cursor = 0;
            self.tx_offset = 0;
        }
    }

    fn deselect_account(&mut self) {
        self.selected_asset_uid = None;
        self.tx_rows = build_tx_rows(collect_tx(self.backup.active_transactions()));
        self.tx_cursor = 0;
        self.tx_offset = 0;
    }
}

/// Collect transactions from an iterator, merging transfer pairs into a single row.
///
/// The DB stores each transfer as two records: TransferOut (DO_TYPE=3) and TransferIn
/// (DO_TYPE=4, the mirror). We keep only the TransferOut side — it already carries
/// `asset_uid` (from) and `to_asset_uid`/`to_asset_name` (to) — and drop TransferIn.
fn collect_tx<'a>(iter: impl Iterator<Item = &'a Transaction>) -> Vec<Transaction> {
    iter.filter(|t| t.transaction_type != TransactionType::TransferIn)
        .cloned()
        .collect()
}

/// Wrap a flat transaction list into `TxRow`s, inserting a `DateHeader` whenever
/// the date changes between consecutive transactions.
fn build_tx_rows(txs: Vec<Transaction>) -> Vec<TxRow> {
    let mut rows: Vec<TxRow> = Vec::new();
    let mut last_date = String::new();
    for tx in txs {
        if tx.date != last_date {
            last_date = tx.date.clone();
            rows.push(TxRow::DateHeader {
                date: last_date.clone(),
            });
        }
        rows.push(TxRow::Tx { tx });
    }
    rows
}

/// Build the grouped account rows from the backup data.
///
/// Reads ZASSETGROUP from the database (via a fresh connection) to get the
/// group ordering and names, then places each active asset under its group.
fn build_account_rows(backup: &MMBakFile) -> Vec<AccountRow> {
    use rusqlite::Connection;

    let mut rows = Vec::new();

    // Try to read groups from the DB so we get the exact app ordering.
    let groups: Vec<(String, String, i64)> = if let Ok(conn) =
        Connection::open(backup.file_path_hint())
    {
        let mut stmt = conn
                .prepare("SELECT ZUID, ZASSETGROUPNAME, ZORDER FROM ZASSETGROUP WHERE ZISDEL = 0 OR ZISDEL IS NULL ORDER BY ZORDER")
                .unwrap_or_else(|_| {
                    // Fallback: no groups at all
                    conn.prepare("SELECT 1 WHERE 0").unwrap()
                });
        stmt.query_map([], |row| {
            let uid: String = row.get(0).unwrap_or_default();
            let name: String = row.get(1).unwrap_or_default();
            let order: i64 = row.get(2).unwrap_or(0);
            Ok((uid, name, order))
        })
        .unwrap()
        .filter_map(|g| g.ok())
        .collect()
    } else {
        Vec::new()
    };

    // Map group UID -> (name, order)
    let mut group_map: HashMap<String, (String, i64)> = HashMap::new();
    for (uid, name, order) in groups {
        group_map.insert(uid, (name, order));
    }

    // Collect active assets, keyed by group UID
    let mut by_group: HashMap<String, Vec<Asset>> = HashMap::new();
    for asset in backup.active_assets() {
        let gid = asset.group_uid.clone().unwrap_or_default();
        by_group.entry(gid).or_default().push(asset.clone());
    }

    // Order group UIDs by the group order field
    let mut group_order: Vec<(String, i64)> = by_group
        .keys()
        .map(|k| {
            let order = group_map.get(k).map(|(_, o)| *o).unwrap_or(999);
            (k.clone(), order)
        })
        .collect();
    group_order.sort_by_key(|(_, o)| *o);

    // Emit rows
    for (gid, _) in group_order {
        let group_name = group_map
            .get(&gid)
            .map(|(n, _)| n.clone())
            .unwrap_or_else(|| "Uncategorized".to_string());

        rows.push(AccountRow::GroupHeader { name: group_name });

        if let Some(assets) = by_group.get(&gid) {
            for asset in assets {
                rows.push(AccountRow::Account {
                    asset: asset.clone(),
                });
            }
        }
    }

    rows
}
