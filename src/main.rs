use std::{fmt::Display, io::BufWriter, str::FromStr};

use anyhow::Context;
use axum::{extract::Extension, routing::get, Router};
use chrono::{DateTime, Utc};
use ics::{components::Property, Event, ICalendar};
use reqwest::cookie::{CookieStore, Jar};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum ReservationStatusCode {
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
    bookings: Vec<BookingWrap>,
    total_count: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BookingWrap {
    booking_status_code: ReservationStatusCode,
    is_completed: bool,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,

    snapshot_json: Booking,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Booking {
    booking_id: i64,
    // business: any
    business_id: i64,
    // business_name: String,
    service_name: String,
    // cancelled_date_time: Option<chrono::DateTime<chrono::FixedOffset>>,
    // completed_date_time: chrono::DateTime<chrono::FixedOffset>,
    // regDatetime: any
    #[serde(rename = "bizItemName")]
    business_item_name: String,
    #[serde(rename = "bizItemId")]
    business_item_id: i64,
    start_date_time: chrono::DateTime<Utc>,
    end_date_time: chrono::DateTime<Utc>,
    business_address_json: Address,
    #[serde(rename = "bookingOptionJson")]
    options: Vec<ReservationOption>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReservationOption {
    name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Address {
    road_addr: String,
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

    let payload = serde_json::json!({
        "operationName": "bookings",
        "variables": {
            "input": {
                "queryType": QueryType(vec![
                    ReservationStatusCode::Cancel,
                    ReservationStatusCode::Completed,
                    ReservationStatusCode::Reserved
                ]),
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
    println!("{:#?}", serde_json::to_string(&payload).unwrap());

    let client = reqwest::Client::new();
    let req = client
        .post(ENDPOINT)
        .header(reqwest::header::COOKIE, jar.cookies(&cookie_url).unwrap())
        .header(reqwest::header::USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/15.5 Safari/605.1.15")
        .json(&payload)
        .build()?;
    println!("{:?}", &req);

    let res = client.execute(req).await?;
    let res = res.bytes().await?;
    let res: NaverCalendarResponse = serde_json::from_slice(&res).with_context(|| {
        format!("Failed to parse\n{}", unsafe {
            std::str::from_utf8_unchecked(&res)
        })
    })?;
    // println!("{:#?}", &res);
    for reservation in res.data.booking.bookings {
        let reservation = reservation.snapshot_json;
        let options = reservation
            .options
            .into_iter()
            .map(|option| option.name)
            .collect::<Vec<_>>();
        let options = unsafe { serde_json::to_string(&options).unwrap_unchecked() };

        sqlx::query!(
            r#"INSERT OR REPLACE INTO
                reservation (
                    `id`,
                    `business_id`,
                    `business_name`,
                    `item_id`,
                    `item_name`,
                    `start_date_time`,
                    `end_date_time`,
                    `options`,
                    `location`
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
            reservation.booking_id,
            reservation.business_id,
            reservation.service_name,
            reservation.business_item_id,
            reservation.business_item_name,
            reservation.start_date_time,
            reservation.end_date_time,
            options,
            reservation.business_address_json.road_addr,
        )
        .execute(&db_pool)
        .await?;
    }

    let app = Router::new()
        .route("/", get(get_all_reservation))
        .layer(Extension(db_pool));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

struct Reservation {
    id: i64,
    business_name: String,
    item_name: String,
    options: Vec<String>,
    start_date_time: DateTime<Utc>,
    end_date_time: DateTime<Utc>,
}

impl sqlx::FromRow<'_, SqliteRow> for Reservation {
    fn from_row(row: &'_ SqliteRow) -> Result<Self, sqlx::Error> {
        let raw_options: String = row.try_get("options")?;

        Ok(Self {
            id: row.try_get("id")?,
            business_name: row.try_get("business_name")?,
            item_name: row.try_get("item_name")?,
            options: serde_json::from_str(&raw_options)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            start_date_time: row.try_get("id")?,
            end_date_time: row.try_get("id")?,
        })
    }
}

impl From<Reservation> for Event<'static> {
    fn from(reservation: Reservation) -> Self {
        let start_date = reservation
            .start_date_time
            .format("%Y%m%dT%H%M%S")
            .to_string();
        let end_date = reservation
            .end_date_time
            .format("%Y%m%dT%H%M%S")
            .to_string();
        let mut event = Event::new(format!("{}", reservation.id), start_date.clone());
        event.push(Property::new(
            "SUMMARY",
            format!("{} - {}", reservation.business_name, reservation.item_name),
        ));
        event.push(Property::new("DESCRIPTION", reservation.options.join("\n")));
        event.push(Property::new("DTSTART", start_date));
        event.push(Property::new("DTEND", end_date));

        event
    }
}

async fn get_all_reservation(Extension(db_pool): Extension<SqlitePool>) -> String {
    let reservations: Vec<Reservation> = sqlx::query_as(
        r#"SELECT
        id, business_name, item_name, options,
        start_date_time,
        end_date_time FROM reservation"#,
    )
    .fetch_all(&db_pool)
    .await
    .unwrap();
    let mut calendar = ICalendar::new("2.0", "kawai");
    for reservation in reservations {
        calendar.add_event(reservation.into());
    }

    let mut writer = BufWriter::new(Vec::new());
    calendar.write(&mut writer).unwrap();
    let buffer = writer.into_inner().unwrap();

    String::from_utf8(buffer).unwrap()
}
