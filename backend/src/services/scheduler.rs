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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Timelike};

    fn make_price(hour: u32, price: f64) -> Price {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let timestamp = date.and_hms_opt(hour, 0, 0).unwrap();
        Price {
            timestamp,
            price,
            source: "test".to_string(),
        }
    }

    #[test]
    fn test_find_cheapest_hours_single_hour() {
        let prices = vec![
            make_price(0, 0.15),
            make_price(1, 0.10), // cheapest
            make_price(2, 0.20),
        ];

        let result = find_cheapest_hours(&prices, 60);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].hour(), 1);
    }

    #[test]
    fn test_find_cheapest_hours_multiple_hours() {
        let prices = vec![
            make_price(0, 0.15),
            make_price(1, 0.10), // 2nd cheapest
            make_price(2, 0.20),
            make_price(3, 0.05), // cheapest
            make_price(4, 0.25),
        ];

        let result = find_cheapest_hours(&prices, 120); // 2 hours

        assert_eq!(result.len(), 2);
        let hours: Vec<u32> = result.iter().map(|t| t.hour()).collect();
        assert!(hours.contains(&1));
        assert!(hours.contains(&3));
    }

    #[test]
    fn test_find_cheapest_hours_rounds_up_duration() {
        let prices = vec![
            make_price(0, 0.15),
            make_price(1, 0.10),
            make_price(2, 0.20),
        ];

        // 90 minutes should round up to 2 hours
        let result = find_cheapest_hours(&prices, 90);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cheapest_hours_empty_prices() {
        let prices: Vec<Price> = vec![];

        let result = find_cheapest_hours(&prices, 60);

        assert!(result.is_empty());
    }

    #[test]
    fn test_find_cheapest_hours_more_hours_than_available() {
        let prices = vec![
            make_price(0, 0.15),
            make_price(1, 0.10),
        ];

        // Request 5 hours but only 2 available
        let result = find_cheapest_hours(&prices, 300);

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cheapest_hours_zero_duration() {
        let prices = vec![
            make_price(0, 0.15),
            make_price(1, 0.10),
        ];

        let result = find_cheapest_hours(&prices, 0);

        assert!(result.is_empty());
    }

    #[test]
    fn test_find_cheapest_hours_equal_prices() {
        let prices = vec![
            make_price(0, 0.10),
            make_price(1, 0.10),
            make_price(2, 0.10),
        ];

        let result = find_cheapest_hours(&prices, 120);

        // Should return 2 hours (any 2 since they're all equal)
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cheapest_hours_full_day() {
        let prices: Vec<Price> = (0..24)
            .map(|h| make_price(h, (24 - h) as f64 * 0.01)) // Hour 23 is cheapest
            .collect();

        let result = find_cheapest_hours(&prices, 180); // 3 hours

        assert_eq!(result.len(), 3);
        let hours: Vec<u32> = result.iter().map(|t| t.hour()).collect();
        assert!(hours.contains(&23));
        assert!(hours.contains(&22));
        assert!(hours.contains(&21));
    }
}
