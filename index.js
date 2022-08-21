// 필요한 것
// 1. 네이버 예약 데이터를 주기적으로 긁어와서 ical로 변환한 뒤 파일로 어딘가에 저장한다  -> 신규파일 생성 계속이 아니라, 기존 것 열어서 rewrite
// 2. 구글(이든 뭐든)이 static serving 되는 ical 파일을 가져와서 알아서 쓴다

import fetch from "node-fetch"
import config from "config"

// 앱이 처음에 딱 실행될때 네이버 세션이 필요함.
const NID_AUT = config.get('NID_AUT');
const NID_SES = config.get("NID_SES");
const cookie_items = [
    `NID_AUT=${NID_AUT}`,
    `NID_SES=${NID_SES}`,
];

const STATUS_CODE = {
    "Cancel": "RC04",
    "Completed": "RC08",
    "Reserved": "RC05",
};

const endpoint = "https://m.booking.naver.com/graphql"

const payload = {
    operationName: "bookings",
    variables: {
        input: {
            queryType: [STATUS_CODE.Cancel, STATUS_CODE.Completed, STATUS_CODE.Reserved].join(","),
            businessMainCategory: "ALL",
            startDate: null,
            endDate: null,
            size: 4,
            page: 0,
        },
    },
    query: `query bookings($input: BookingParams) {
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
`,
}
const serialized_payload = JSON.stringify(payload);
// 일단 최근 4개거 갖고올수잇는지 함 봅시다

(async function() {
    const res = await fetch("https://m.booking.naver.com/graphql", {
        "headers": {
            "content-type": "application/json",
            "cookie": cookie_items.join("; "),
        },
        "body": serialized_payload,
        "method": "POST"
    });
    const body = await res.json();
    console.log(body);
})();
