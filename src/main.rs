mod naver_reservation;

use std::{fmt::Display, io::BufWriter};

use anyhow::Context;
use axum::{extract::Extension, response::IntoResponse, routing::get, Router};
use chrono::{DateTime, NaiveDateTime, Utc};
use ics::{components::Property, Event, ICalendar};
use naver_reservation::{FetchOption, UserAuth};
use reqwest::{header, StatusCode};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

type DbExecutor = sqlx::SqlitePool;

static USER_AUTH: once_cell::sync::OnceCell<UserAuth> = once_cell::sync::OnceCell::new();

async fn poll_naver_reservation(db: DbExecutor) -> anyhow::Result<()> {
    use naver_reservation::ReservationStatusCode;

    let res = naver_reservation::fetch(
        unsafe { USER_AUTH.get_unchecked() },
        FetchOption {
            query_types: vec![
                ReservationStatusCode::Cancel,
                ReservationStatusCode::Completed,
                ReservationStatusCode::Reserved,
            ],
            size: 10,
            ..Default::default()
        },
    )
    .await?;

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
        .execute(&db)
        .await?;
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let db_pool = sqlx::SqlitePool::connect("./data.sqlite").await?;
    sqlx::migrate!().run(&db_pool).await?;

    let user = UserAuth {
        aut: std::env::var("NID_AUT")?,
        ses: std::env::var("NID_SES")?,
    };
    if USER_AUTH.set(user).is_err() {
        return Err(anyhow::anyhow!("Impossible initialization logic error"));
    }

    let app = Router::new()
        .route("/", get(get_all_reservation))
        .route("/fetch", get(fetch_naver_reservation))
        .layer(Extension(db_pool));

    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();

    Ok(())
}

async fn fetch_naver_reservation(Extension(db_pool): Extension<SqlitePool>) {
    if let Err(e) = poll_naver_reservation(db_pool).await {
        log::error!("Failed to fetch naver reservation - {}", e);
    }
}

#[derive(Debug)]
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

        let start_date_time: NaiveDateTime = row.try_get("start_date_time")?;
        let end_date_time: NaiveDateTime = row.try_get("end_date_time")?;
        Ok(Self {
            id: row.try_get("id")?,
            business_name: row.try_get("business_name")?,
            item_name: row.try_get("item_name")?,
            options: serde_json::from_str(&raw_options)
                .map_err(|e| sqlx::Error::Decode(Box::new(e)))?,
            start_date_time: DateTime::from_utc(start_date_time, Utc),
            end_date_time: DateTime::from_utc(end_date_time, Utc),
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
        event.push(Property::new("DTSTART;TZID=Etc/UTC", start_date));
        event.push(Property::new("DTEND;TZID=Etc/UTC", end_date));

        event
    }
}

async fn reservation_to_ics(db_pool: DbExecutor) -> anyhow::Result<String> {
    let reservations: Vec<Reservation> = sqlx::query_as(
        r#"SELECT
        id, business_name, item_name, options,
        start_date_time,
        end_date_time FROM reservation"#,
    )
    .fetch_all(&db_pool)
    .await?;
    let mut calendar = ICalendar::new("2.0", "kawai");
    calendar.push(Property::new("X-WR-TIMEZONE", "Etc/UTC"));
    calendar.add_timezone(ics::TimeZone::standard(
        "Etc/UTC",
        ics::Standard::new("19700101T000000", "0000", "0000"),
    ));
    for reservation in reservations {
        calendar.add_event(reservation.into());
    }

    let mut writer = BufWriter::new(Vec::new());
    calendar.write(&mut writer).unwrap();
    let buffer = writer.into_inner()?;

    Ok(String::from_utf8(buffer)?)
}

async fn get_all_reservation(Extension(db_pool): Extension<SqlitePool>) -> impl IntoResponse {
    match reservation_to_ics(db_pool).await {
        Ok(body) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/calendar")],
            body,
        ),
        Err(e) => {
            log::error!("Failed to generate ics - {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(header::CONTENT_TYPE, "text/plain")],
                "".to_string(),
            )
        }
    }
}
