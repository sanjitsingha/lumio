use crate::currency::{self, Rates};

/// The saved exchange rates, if any have been downloaded yet.
#[tauri::command]
pub fn get_currency_rates() -> Option<Rates> {
    currency::rates()
}
