use std::str::FromStr;

use reqwest::cookie::{CookieStore, Jar};

mod reservation_status_code {
    pub const CANCEL: &str = "RC04";
    pub const COMPLETED: &str = "RC08";
    pub const RESERVED: &str = "RC05";
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

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
    println!("{:?}", &req);

    let res = client.execute(req).await?;
    println!("{}", res.json::<serde_json::Value>().await?);
    Ok(())
}
