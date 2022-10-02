use std::str::FromStr;

use chrono::Datelike;
use reqwest::cookie::{CookieStore, Jar};
use serde_with::serde_as;

mod reservation_status_code {
    pub const CANCEL: &str = "RC04";
    pub const COMPLETED: &str = "RC08";
    pub const RESERVED: &str = "RC05";
}

#[derive(Debug, Clone, serde::Deserialize)]
struct NaverCalendarResponse {
    data: Data,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct Data {
    booking: Booking2,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Booking2 {
    bookings: Vec<Booking>,
    total_count: u32,
}

#[serde_as]
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Booking {
    #[serde_as(as = "serde_with::DisplayFromStr")]
    booking_id: i64,
    booking_status_code: String,
    // business: any
    business_name: String,
    cancelled_date_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    completed_date_time: chrono::DateTime<chrono::FixedOffset>,
    end_date: chrono::NaiveDate,
    is_completed: bool,
    // regDatetime: any
    service_name: String,
    start_date: chrono::NaiveDate,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_pool = sqlx::SqlitePool::connect("./data.sqlite").await?;
    sqlx::migrate!().run(&db_pool).await?;

    const ENDPOINT: &str = "https://m.booking.naver.com/graphql";
    let cookie_url = reqwest::Url::from_str(ENDPOINT).unwrap();
    let jar = Jar::default();
    for cookie_name in ["NID_AUT", "NID_SES"] {
        let value = std::env::var(cookie_name)?;
        let cookie = format!("{}={}", cookie_name, value);
        jar.add_cookie_str(&cookie, &cookie_url);
    }

    let query_type = [
        reservation_status_code::CANCEL,
        reservation_status_code::COMPLETED,
        reservation_status_code::RESERVED,
    ]
    .join(",");
    let payload = serde_json::json!({
        "operationName": "bookings",
        "variables": {
            "input": {
                "queryType": query_type,
                "businessMainCategory": "ALL",
                "startDate": null,
                "endDate": null,
                "size": 4,
                "page": 0,
            },
        },
        "query": r#"query bookings($input: BookingParams) {
            booking(input: $input) {
              id
              totalCount
              bookings {
                bookingId
                businessName
                serviceName
                bookingStatusCode
                isCompleted
                startDate
                endDate
                regDateTime
                completedDateTime
                cancelledDateTime
                business {
                  addressJson
                  completedPinValue
                  name
                  serviceName
                  isImp
                  isDeleted
                  isCompletedButtonImp
                  phoneInformationJson
                }
              }
            }
          }
          "#,
    });

    let client = reqwest::Client::new();
    let req = client
        .post(ENDPOINT)
        .header(reqwest::header::COOKIE, jar.cookies(&cookie_url).unwrap())
        .json(&payload)
        .build()?;
    // println!("{:?}", &req);

    let tz = chrono::FixedOffset::east(9 * 3600);

    let res = client.execute(req).await?;
    let res = res.json::<NaverCalendarResponse>().await?;
    // println!("{:#?}", );
    for reservation in res.data.booking.bookings {
        let id = reservation.booking_id as i64;
        let name = reservation.service_name;
        let start_date = reservation.start_date;
        let end_date = reservation.end_date;
        sqlx::query!(
            "INSERT OR REPLACE INTO reservation (`id`, `name`, `start_date`, `end_date`) VALUES (?, ?, ?, ?)",
            id,
            name,
            start_date,
            end_date
        )
        .execute(&db_pool)
        .await?;
    }

    Ok(())
}
