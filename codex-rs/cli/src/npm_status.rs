use anyhow::Context;
use chrono::Duration;
use chrono::NaiveDate;
use chrono::Utc;
use serde::Deserialize;

const NPM_DOWNLOADS_BASE_URL: &str = "https://api.npmjs.org/downloads/point";
const OPEN_CODEX_NPM_PACKAGE: &str = "@leonw24/open-codex";
const NPM_DOWNLOADS_EARLIEST_DATE: &str = "2015-01-10";
const MAX_POINT_RANGE_DAYS: i64 = 365;

#[derive(Debug, Deserialize)]
struct PointDownloadsResponse {
    downloads: u64,
    start: String,
    end: String,
    package: Option<String>,
}

#[derive(Debug)]
struct DownloadStatus {
    package: String,
    weekly_downloads: u64,
    weekly_start: String,
    weekly_end: String,
    total_downloads: u64,
    total_start: NaiveDate,
    total_end: NaiveDate,
}

pub(crate) async fn print_npm_status() -> anyhow::Result<()> {
    let status = fetch_download_status(OPEN_CODEX_NPM_PACKAGE).await?;

    println!("npm package: {}", status.package);
    println!(
        "weekly downloads: {} ({}..{})",
        format_count(status.weekly_downloads),
        status.weekly_start,
        status.weekly_end
    );
    println!(
        "total downloads: {} ({}..{})",
        format_count(status.total_downloads),
        status.total_start,
        status.total_end
    );
    println!("source: {NPM_DOWNLOADS_BASE_URL}");

    Ok(())
}

async fn fetch_download_status(package: &str) -> anyhow::Result<DownloadStatus> {
    let client = reqwest::Client::new();
    let weekly = fetch_point_downloads(&client, "last-week", package)
        .await
        .context("failed to fetch weekly npm downloads")?;
    let today = Utc::now().date_naive();
    let total_start = NaiveDate::parse_from_str(NPM_DOWNLOADS_EARLIEST_DATE, "%Y-%m-%d")
        .context("invalid npm downloads earliest date")?;
    let total = fetch_total_downloads(&client, total_start, today, package)
        .await
        .context("failed to fetch total npm downloads")?;

    Ok(DownloadStatus {
        package: weekly
            .package
            .unwrap_or_else(|| OPEN_CODEX_NPM_PACKAGE.to_string()),
        weekly_downloads: weekly.downloads,
        weekly_start: weekly.start,
        weekly_end: weekly.end,
        total_downloads: total,
        total_start,
        total_end: today,
    })
}

async fn fetch_total_downloads(
    client: &reqwest::Client,
    start: NaiveDate,
    end: NaiveDate,
    package: &str,
) -> anyhow::Result<u64> {
    let mut total = 0;
    for (range_start, range_end) in point_ranges(start, end) {
        let period = format!("{range_start}:{range_end}");
        let chunk = fetch_point_downloads(client, &period, package).await?;
        total += chunk.downloads;
    }
    Ok(total)
}

async fn fetch_point_downloads(
    client: &reqwest::Client,
    period: &str,
    package: &str,
) -> anyhow::Result<PointDownloadsResponse> {
    let url = format!("{NPM_DOWNLOADS_BASE_URL}/{period}/{package}");
    let response = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("request failed for {url}"))?
        .error_for_status()
        .with_context(|| format!("npm downloads API returned an error for {url}"))?;

    response
        .json::<PointDownloadsResponse>()
        .await
        .with_context(|| format!("failed to parse npm downloads response from {url}"))
}

fn point_ranges(start: NaiveDate, end: NaiveDate) -> Vec<(NaiveDate, NaiveDate)> {
    if start > end {
        return Vec::new();
    }

    let mut ranges = Vec::new();
    let mut range_start = start;
    while range_start <= end {
        let range_end = std::cmp::min(range_start + Duration::days(MAX_POINT_RANGE_DAYS - 1), end);
        ranges.push((range_start, range_end));
        range_start = range_end + Duration::days(1);
    }
    ranges
}

fn format_count(value: u64) -> String {
    let raw = value.to_string();
    let mut formatted = String::with_capacity(raw.len() + raw.len() / 3);
    let first_group_len = match raw.len() % 3 {
        0 => 3,
        len => len,
    };

    for (index, ch) in raw.chars().enumerate() {
        if index > 0
            && (index == first_group_len
                || (index > first_group_len && (index - first_group_len) % 3 == 0))
        {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_ranges_cover_dates_without_exceeding_limit() {
        let start = NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date");
        let end = NaiveDate::from_ymd_opt(2025, 1, 3).expect("valid date");

        let ranges = point_ranges(start, end);

        assert_eq!(
            ranges,
            vec![
                (
                    NaiveDate::from_ymd_opt(2024, 1, 1).expect("valid date"),
                    NaiveDate::from_ymd_opt(2024, 12, 30).expect("valid date"),
                ),
                (
                    NaiveDate::from_ymd_opt(2024, 12, 31).expect("valid date"),
                    NaiveDate::from_ymd_opt(2025, 1, 3).expect("valid date"),
                ),
            ]
        );
        assert!(ranges.iter().all(|(range_start, range_end)| {
            (*range_end - *range_start).num_days() < MAX_POINT_RANGE_DAYS
        }));
    }

    #[test]
    fn format_count_inserts_group_separators() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(12_345_678), "12,345,678");
    }
}
