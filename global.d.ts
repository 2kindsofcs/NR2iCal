const enum BookingStatusCode {
    Cancel = "RC04",
    Completed = "RC08",
    Reserved = "RC05"
}

interface NaverReservationResponse {
    data: {
        booking: {
            id: null;
            totalCount: number;
            bookings: Booking[];
        }
    }
}
interface Booking {
    bookingId: string;
    businessName: string;
    serviceName: string;
    bookingStatusCode: BookingStatusCode;
    isCompleted: boolean;
    startDate: string;
    endDate: string;
    regDateTime: string;
    completedDateTime: string;
    cancelledDateTime?: any;
    business: Business | null;
}

interface Business {
    addressJson: AddressJson;
    completedPinValue?: any;
    name: string;
    serviceName: string;
    isImp: boolean;
    isDeleted: boolean;
    isCompletedButtonImp: boolean;
    phoneInformationJson: PhoneInformationJson;
}

interface PhoneInformationJson {
    phoneList: any[];
    wiredPhone?: any;
    reprPhone: string;
}

interface AddressJson {
    jibun: string;
    roadAddr: string;
    posLong: number;
    posLat: number;
    zoomLevel: number;
    address: string;
    detail: string;
}