use crate::models::Price;
use chrono::NaiveDateTime;

pub fn find_cheapest_hours(prices: &[Price], duration_minutes: i32) -> Vec<NaiveDateTime> {
    // Basic logic: Pick the N cheapest hours.
    // Improve later for contiguous blocks if needed.

    let mut sorted_prices = prices.to_vec();
    sorted_prices.sort_by(|a, b| a.price.partial_cmp(&b.price).unwrap());

    let hours_needed = (duration_minutes as f64 / 60.0).ceil() as usize;

    // Take needed hours, strictly creating a schedule.
    // In reality we might want to return the PriceData or something to schedule it.
    // Returning timestamps for now.
    sorted_prices
        .into_iter()
        .take(hours_needed)
        .map(|p| p.timestamp)
        .collect()
}
