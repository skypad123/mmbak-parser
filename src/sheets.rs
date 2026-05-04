use std::path::Path;

use google_sheets4::{api::ValueRange, hyper_rustls, hyper_util, yup_oauth2, Sheets};

use crate::error::{MMBakError, Result};

fn format_google_error(e: &google_sheets4::Error) -> String {
    let body_str = match e {
        google_sheets4::Error::BadRequest(body) => body.to_string(),
        google_sheets4::Error::Failure(resp) => {
            // Try to read the body from the response
            format!("HTTP {}", resp.status())
        }
        _ => return e.to_string(),
    };

    if body_str.contains("PERMISSION_DENIED") || body_str.contains("permission") {
        format!(
            "Permission denied.\n\n\
            The service account does not have access to this spreadsheet.\n\
            To fix: open the Google Sheet, click Share, and add the service\n\
            account email (found in the JSON key's 'client_email' field)\n\
            with Editor permissions.\n\n\
            Raw response: {body_str}"
        )
    } else {
        format!("{e}")
    }
}

/// Append rows to a Google Sheet.
///
/// Authenticates with the service account key, builds a `google_sheets4` hub,
/// and appends the rows in a single API call.
///
/// `service_account_path` — path to the service-account JSON key file.
/// `spreadsheet_id` — the Google Sheet ID.
/// `range` — e.g. `"Entity_Log"` or `"Entity_Log!A:F"`.
/// `values` — each inner vector is one row.
pub async fn append_values(
    service_account_path: &Path,
    spreadsheet_id: &str,
    range: &str,
    values: Vec<Vec<String>>,
) -> Result<()> {
    // --- authenticate ---
    let key = yup_oauth2::read_service_account_key(service_account_path)
        .await
        .map_err(|e| {
            MMBakError::GoogleApiError(format!("Failed to read service account key: {e}"))
        })?;

    let auth = yup_oauth2::ServiceAccountAuthenticator::builder(key)
        .build()
        .await
        .map_err(|e| {
            MMBakError::GoogleApiError(format!("Failed to build authenticator: {e}"))
        })?;

    let connector = hyper_rustls::HttpsConnectorBuilder::new()
        .with_native_roots()
        .unwrap()
        .https_or_http()
        .enable_http2()
        .build();

    let client = hyper_util::client::legacy::Client::builder(
        hyper_util::rt::TokioExecutor::new(),
    )
    .build(connector);

    let hub = Sheets::new(client, auth);

    // --- build request ---
    let json_values: Vec<Vec<serde_json::Value>> = values
        .into_iter()
        .map(|row| row.into_iter().map(serde_json::Value::String).collect())
        .collect();

    let value_range = ValueRange {
        values: Some(json_values),
        range: Some(range.to_string()),
        ..Default::default()
    };

    // --- execute ---
    let result = hub
        .spreadsheets()
        .values_append(value_range, spreadsheet_id, range)
        .value_input_option("USER_ENTERED")
        .doit()
        .await;

    match result {
        Ok(_) => Ok(()),
        Err(ref e) => Err(MMBakError::GoogleApiError(format!(
            "Sheets API append failed: {}",
            format_google_error(e)
        ))),
    }
}
