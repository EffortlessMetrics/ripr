pub async fn fetch_limit(capacity: i32, threshold: i32) -> i32 {
    if capacity > threshold { capacity - 10 } else { capacity }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn above_threshold_gets_reduced() {
        let result = fetch_limit(100, 50).await;
        assert_eq!(result, 90);
    }
}
