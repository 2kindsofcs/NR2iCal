use std::fmt::Display;

use anyhow::Context;
use chrono::{DateTime, NaiveDate, Utc};
use reqwest::cookie::{CookieStore, Jar};

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub enum ReservationStatusCode {
    #[serde(rename = "RC04")]
    Cancel,
    #[serde(rename = "RC08")]
    Completed,
    #[serde(rename = "RC05")]
    Reserved,
}

impl Display for ReservationStatusCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stringified = unsafe { serde_json::to_string(self).unwrap_unchecked() };
        write!(f, "{}", stringified.trim_matches('"'))
    }
}

#[serde_with::serde_as]
#[derive(serde::Serialize)]
struct QueryType(
    #[serde_as(
        as = "serde_with::StringWithSeparator::<serde_with::formats::CommaSeparator, ReservationStatusCode>"
    )]
    Vec<ReservationStatusCode>,
);

#[derive(Debug, Clone, serde::Deserialize)]
pub struct NaverCalendarResponse {
    pub data: Data,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Data {
    pub booking: Booking2,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Booking2 {
    pub bookings: Vec<BookingWrap>,
    pub total_count: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookingWrap {
    pub booking_status_code: ReservationStatusCode,
    pub is_completed: bool,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,

    pub snapshot_json: Booking,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Booking {
    pub booking_id: i64,
    // pub business: any
    pub business_id: i64,
    // pub business_name: String,
    pub service_name: String,
    // pub cancelled_date_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    // pub completed_date_time: chrono::DateTime<chrono::FixedOffset>,
    // pub regDatetime: any
    #[serde(rename = "bizItemName")]
    pub business_item_name: String,
    #[serde(rename = "bizItemId")]
    pub business_item_id: i64,
    pub start_date_time: DateTime<Utc>,
    pub end_date_time: DateTime<Utc>,
    pub business_address_json: Address,
    #[serde(rename = "bookingOptionJson")]
    pub options: Vec<ReservationOption>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReservationOption {
    pub name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Address {
    pub road_addr: String,
}

static ENDPOINT: once_cell::sync::Lazy<reqwest::Url> = once_cell::sync::Lazy::new(|| {
    use std::str::FromStr;
    unsafe { reqwest::Url::from_str("https://m.booking.naver.com/graphql").unwrap_unchecked() }
});

pub struct UserAuth {
    pub aut: String,
    pub ses: String,
}

impl UserAuth {
    fn to_cookie_jar(&self) -> Jar {
        let jar = Jar::default();
        jar.add_cookie_str(&format!("{}={}", "NID_AUT", self.aut), &ENDPOINT);
        jar.add_cookie_str(&format!("{}={}", "NID_SES", self.ses), &ENDPOINT);
        jar
    }
}

#[derive(Debug, Default, Clone)]
pub struct FetchOption {
    pub query_types: Vec<ReservationStatusCode>,
    pub start_date: Option<DateTime<Utc>>,
    pub end_date: Option<DateTime<Utc>>,
    pub size: usize,
    pub page: usize,
}

pub async fn fetch(
    user_auth: &UserAuth,
    option: FetchOption,
) -> anyhow::Result<NaverCalendarResponse> {
    let payload = serde_json::json!({
        "operationName": "bookings",
        "variables": {
            "input": {
                "queryType": QueryType(option.query_types),
                "businessMainCategory": "ALL",
                "startDate": option.start_date,
                "endDate": option.end_date,
                "size": option.size,
                "page": option.page,
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
                snapshotJson
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

    let jar = user_auth.to_cookie_jar();

    let client = reqwest::Client::new();
    let req = client
        .post(ENDPOINT.as_ref())
        .header(reqwest::header::COOKIE, jar.cookies(&ENDPOINT).unwrap())
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.5 Safari/605.1.15")
        .json(&payload)
        .build()?;

    let res = client.execute(req).await?;
    let res = res.bytes().await?;
    let res: NaverCalendarResponse = serde_json::from_slice(&res).with_context(|| {
        format!("Failed to parse\n{}", unsafe {
            std::str::from_utf8_unchecked(&res)
        })
    })?;

    Ok(res)
}
