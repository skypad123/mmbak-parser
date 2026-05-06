use std::fmt;

/// Transaction type (income, expense, or transfer)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    /// Expense transaction
    Expense,
    /// Income transaction
    Income,
    /// Transfer between accounts
    Transfer,
}

impl TransactionType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "E" => Some(TransactionType::Expense),
            "I" => Some(TransactionType::Income),
            "T" => Some(TransactionType::Transfer),
            _ => None,
        }
    }

    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(TransactionType::Income),
            1 => Some(TransactionType::Expense),
            3 | 4 => Some(TransactionType::Transfer),
            7 => Some(TransactionType::Income), // Market value increase
            8 => Some(TransactionType::Expense), // Market value decrease
            _ => None,
        }
    }
}

impl fmt::Display for TransactionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransactionType::Expense => write!(f, "Expense"),
            TransactionType::Income => write!(f, "Income"),
            TransactionType::Transfer => write!(f, "Transfer"),
        }
    }
}

/// A transaction (income/expense entry) from ZINOUTCOME table
#[derive(Debug, Clone)]
pub struct Transaction {
    pub uid: String,
    pub transaction_type: TransactionType,
    /// Raw DO_TYPE value from the database (0=Income, 1=Expense, 3=TransferOut, 4=TransferIn, 7/8=ModifiedBal)
    pub raw_do_type: i64,
    pub amount: f64,
    pub date: String,
    pub category_uid: Option<String>,
    pub category_name: Option<String>,
    pub asset_uid: Option<String>,
    pub asset_name: Option<String>,
    pub to_asset_uid: Option<String>,
    pub currency_uid: Option<String>,
    pub memo: Option<String>,
    pub payee: Option<String>,
    pub is_deleted: bool,
}

impl Transaction {
    /// Format amount as a string with currency symbol
    pub fn format_amount(&self) -> String {
        format!("{:.2}", self.amount)
    }

    /// Get a short description of the transaction
    pub fn description(&self) -> String {
        if let Some(memo) = &self.memo {
            if !memo.is_empty() {
                return memo.clone();
            }
        }
        if let Some(payee) = &self.payee {
            if !payee.is_empty() {
                return payee.clone();
            }
        }
        if let Some(category) = &self.category_name {
            return category.clone();
        }
        "No description".to_string()
    }

    /// Returns true if this is the "destination" side of a transfer pair
    /// (DO_TYPE=4).  These records are mirrors of the DO_TYPE=3 source
    /// side and must be skipped during balance calculation.
    pub fn is_transfer_in(&self) -> bool {
        self.raw_do_type == 4
    }
}

/// An account/asset from ZASSET table
#[derive(Debug, Clone)]
pub struct Asset {
    pub uid: String,
    pub name: String,
    pub nicname: Option<String>,
    pub asset_type: i64,
    pub left_money: i64,
    pub calculated_balance: f64,
    pub currency_uid: Option<String>,
    pub group_uid: Option<String>,
    pub memo: Option<String>,
    pub is_deleted: bool,
}

impl Asset {
    /// Get the display name (prefer nicname if available)
    pub fn display_name(&self) -> &str {
        self.nicname.as_ref().unwrap_or(&self.name)
    }

    /// Format the calculated balance
    pub fn format_balance(&self) -> String {
        format!("{:.2}", self.calculated_balance)
    }
}

/// A category from ZCATEGORY table
#[derive(Debug, Clone)]
pub struct Category {
    pub uid: String,
    pub name: String,
    pub parent_uid: Option<String>,
    pub do_type: i64, // Type: income or expense
    pub status: i64,
    pub is_deleted: bool,
}

/// A currency from ZCURRENCY table
#[derive(Debug, Clone)]
pub struct Currency {
    pub uid: String,
    pub iso: String,
    pub symbol: Option<String>,
    pub rate: f64,
    pub is_main_currency: bool,
    pub is_show: bool,
}

/// Database statistics
#[derive(Debug, Clone)]
pub struct DatabaseStats {
    pub total_transactions: usize,
    pub total_assets: usize,
    pub total_categories: usize,
    pub total_currencies: usize,
    pub active_transactions: usize,
    pub active_assets: usize,
    pub total_income: f64,
    pub total_expense: f64,
}

impl DatabaseStats {
    pub fn net_balance(&self) -> f64 {
        self.total_income - self.total_expense
    }
}

/// Format byte size to human-readable string
pub fn format_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    if size == 0 {
        return "0 B".to_string();
    }
    let exp = (size as f64).log(1024.0).min(UNITS.len() as f64 - 1.0) as usize;
    let value = size as f64 / 1024f64.powi(exp as i32);
    if exp == 0 {
        format!("{} {}", size, UNITS[0])
    } else {
        format!("{:.2} {}", value, UNITS[exp])
    }
}

/// Format money amount to human-readable string
pub fn format_money(amount: f64) -> String {
    if amount.abs() < 1_000.0 {
        format!("{:.2}", amount)
    } else if amount.abs() < 1_000_000.0 {
        format!("{:.2}K", amount / 1_000.0)
    } else if amount.abs() < 1_000_000_000.0 {
        format!("{:.2}M", amount / 1_000_000.0)
    } else {
        format!("{:.2}B", amount / 1_000_000_000.0)
    }
}
