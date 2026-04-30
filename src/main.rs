use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};
use colored::Colorize;

use mmbak_parser::{MMBakFile, Result};

#[derive(Parser)]
#[command(name = "mmbak")]
#[command(about = "MoneyManager backup file parser and extractor")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Show backup information
    Info {
        /// Backup file to inspect
        file: PathBuf,
    },

    /// List transactions
    Transactions {
        /// Backup file to read
        file: PathBuf,

        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,

        /// Filter by transaction type (income, expense, transfer)
        #[arg(short = 't', long)]
        filter_type: Option<String>,
    },

    /// List accounts/assets
    Accounts {
        /// Backup file to read
        file: PathBuf,

        /// Show detailed information
        #[arg(short, long)]
        detailed: bool,
    },

    /// List categories
    Categories {
        /// Backup file to read
        file: PathBuf,
    },

    /// Show statistics
    Stats {
        /// Backup file to analyze
        file: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Commands::Info { file } => show_info(file),
        Commands::Transactions {
            file,
            detailed,
            limit,
            filter_type,
        } => list_transactions(file, detailed, limit, filter_type),
        Commands::Accounts { file, detailed } => list_accounts(file, detailed),
        Commands::Categories { file } => list_categories(file),
        Commands::Stats { file } => show_stats(file),
    };

    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}

fn show_info(path: PathBuf) -> Result<()> {
    let backup = MMBakFile::open(&path)?;
    let stats = backup.stats();

    println!("{}", "MoneyManager Backup Information".bold().underline());
    println!("  {:20} {}", "File:", path.display().to_string().cyan());
    println!(
        "  {:20} {}",
        "File size:",
        mmbak_parser::types::format_size(backup.file_size())
    );
    println!();
    
    println!("{}", "Contents".bold().underline());
    println!(
        "  {:20} {} ({} active)",
        "Transactions:",
        stats.total_transactions,
        stats.active_transactions
    );
    println!(
        "  {:20} {} ({} active)",
        "Accounts:",
        stats.total_assets,
        stats.active_assets
    );
    println!("  {:20} {}", "Categories:", stats.total_categories);
    println!("  {:20} {}", "Currencies:", stats.total_currencies);
    println!();

    println!("{}", "Financial Summary".bold().underline());
    println!(
        "  {:20} {}",
        "Total Income:",
        format!("{:.2}", stats.total_income).green()
    );
    println!(
        "  {:20} {}",
        "Total Expense:",
        format!("{:.2}", stats.total_expense).red()
    );
    println!(
        "  {:20} {}",
        "Net Balance:",
        if stats.net_balance() >= 0.0 {
            format!("{:.2}", stats.net_balance()).green()
        } else {
            format!("{:.2}", stats.net_balance()).red()
        }
    );

    Ok(())
}

fn list_transactions(
    path: PathBuf,
    detailed: bool,
    limit: Option<usize>,
    filter_type: Option<String>,
) -> Result<()> {
    let backup = MMBakFile::open(&path)?;

    let mut transactions: Vec<_> = backup.active_transactions().collect();

    // Apply filter
    if let Some(filter) = filter_type {
        let filter_lower = filter.to_lowercase();
        transactions.retain(|tx| {
            tx.transaction_type.to_string().to_lowercase().contains(&filter_lower)
        });
    }

    // Apply limit
    let display_transactions = if let Some(n) = limit {
        &transactions[..n.min(transactions.len())]
    } else {
        &transactions[..]
    };

    if detailed {
        println!(
            "{:10} {:8} {:>15} {:20} {:20} {}",
            "Date".bold(),
            "Type".bold(),
            "Amount".bold(),
            "Category".bold(),
            "Account".bold(),
            "Description".bold()
        );
        println!("{}", "─".repeat(100));

        for tx in display_transactions {
            let type_str = match tx.transaction_type {
                mmbak_parser::TransactionType::Income => "Income".green(),
                mmbak_parser::TransactionType::Expense => "Expense".red(),
                mmbak_parser::TransactionType::Transfer => "Transfer".yellow(),
            };

            let amount_str = if tx.transaction_type == mmbak_parser::TransactionType::Expense {
                format!("{:>15.2}", tx.amount).red()
            } else {
                format!("{:>15.2}", tx.amount).green()
            };

            let category = tx.category_name.as_deref().unwrap_or("-");
            let account = tx.asset_name.as_deref().unwrap_or("-");

            println!(
                "{:10} {:8} {} {:20} {:20} {}",
                &tx.date[..10.min(tx.date.len())],
                type_str,
                amount_str,
                truncate(category, 20),
                truncate(account, 20),
                truncate(&tx.description(), 30)
            );
        }
    } else {
        for tx in display_transactions {
            let type_symbol = match tx.transaction_type {
                mmbak_parser::TransactionType::Income => "+".green(),
                mmbak_parser::TransactionType::Expense => "-".red(),
                mmbak_parser::TransactionType::Transfer => "→".yellow(),
            };

            println!(
                "{} {} {} {}",
                &tx.date[..10.min(tx.date.len())],
                type_symbol,
                format!("{:>10.2}", tx.amount),
                tx.description()
            );
        }
    }

    println!();
    println!("Total: {} transactions", display_transactions.len());

    Ok(())
}

fn list_accounts(path: PathBuf, detailed: bool) -> Result<()> {
    let backup = MMBakFile::open(&path)?;

    let accounts: Vec<_> = backup.active_assets().collect();

    if detailed {
        println!(
            "{:30} {:>15} {:10} {}",
            "Name".bold(),
            "Balance".bold(),
            "Type".bold(),
            "UID".bold()
        );
        println!("{}", "─".repeat(80));

        for account in &accounts {
            let uid_display = if account.uid.len() >= 8 {
                &account.uid[..8]
            } else {
                &account.uid
            };

            let balance_str = if account.calculated_balance >= 0.0 {
                format!("{:>15.2}", account.calculated_balance).green()
            } else {
                format!("{:>15.2}", account.calculated_balance).red()
            };

            println!(
                "{:30} {} {:10} {}",
                truncate(account.display_name(), 30),
                balance_str,
                account.asset_type,
                uid_display
            );
        }
    } else {
        for account in &accounts {
            let balance_str = if account.calculated_balance >= 0.0 {
                format!("{:>15.2}", account.calculated_balance).green()
            } else {
                format!("{:>15.2}", account.calculated_balance).red()
            };

            println!(
                "{:30} {}",
                account.display_name(),
                balance_str
            );
        }
    }

    // Calculate total balance
    let total_balance: f64 = accounts.iter().map(|a| a.calculated_balance).sum();

    println!();
    println!("Total: {} accounts", accounts.len());
    println!(
        "Combined Balance: {}",
        if total_balance >= 0.0 {
            format!("{:.2}", total_balance).green()
        } else {
            format!("{:.2}", total_balance).red()
        }
    );

    Ok(())
}

fn list_categories(path: PathBuf) -> Result<()> {
    let backup = MMBakFile::open(&path)?;

    let categories: Vec<_> = backup.active_categories().collect();

    println!(
        "{:30} {:10} {}",
        "Name".bold(),
        "Type".bold(),
        "UID".bold()
    );
    println!("{}", "─".repeat(60));

    for category in &categories {
        let type_str = if category.do_type == 0 {
            "Income".green()
        } else {
            "Expense".red()
        };

        let uid_display = if category.uid.len() >= 8 {
            &category.uid[..8]
        } else {
            &category.uid
        };

        println!(
            "{:30} {:10} {}",
            truncate(&category.name, 30),
            type_str,
            uid_display
        );
    }

    println!();
    println!("Total: {} categories", categories.len());

    Ok(())
}

fn show_stats(path: PathBuf) -> Result<()> {
    let backup = MMBakFile::open(&path)?;
    let stats = backup.stats();

    println!("{}", "Database Statistics".bold().underline());
    println!();

    println!("{}", "Record Counts".bold());
    println!("  {:25} {}", "Total transactions:", stats.total_transactions);
    println!("  {:25} {}", "Active transactions:", stats.active_transactions);
    println!("  {:25} {}", "Deleted transactions:", stats.total_transactions - stats.active_transactions);
    println!("  {:25} {}", "Total accounts:", stats.total_assets);
    println!("  {:25} {}", "Active accounts:", stats.active_assets);
    println!("  {:25} {}", "Total categories:", stats.total_categories);
    println!("  {:25} {}", "Total currencies:", stats.total_currencies);
    println!();

    println!("{}", "Financial Overview".bold());
    println!(
        "  {:25} {}",
        "Total Income:",
        format!("{:.2}", stats.total_income).green()
    );
    println!(
        "  {:25} {}",
        "Total Expense:",
        format!("{:.2}", stats.total_expense).red()
    );
    println!(
        "  {:25} {}",
        "Net Balance:",
        if stats.net_balance() >= 0.0 {
            format!("{:.2}", stats.net_balance()).green()
        } else {
            format!("{:.2}", stats.net_balance()).red()
        }
    );
    println!();

    println!("{}", "Top Categories by Transaction Count".bold());
    let mut category_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for tx in backup.active_transactions() {
        if let Some(cat) = &tx.category_name {
            *category_counts.entry(cat.clone()).or_insert(0) += 1;
        }
    }
    let mut category_vec: Vec<_> = category_counts.into_iter().collect();
    category_vec.sort_by(|a, b| b.1.cmp(&a.1));
    
    for (i, (cat, count)) in category_vec.iter().take(10).enumerate() {
        println!("  {}. {:30} {}", i + 1, truncate(cat, 30), count);
    }

    Ok(())
}

/// Helper to truncate strings with ellipsis
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        s[..max_len].to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
