#[macro_use]
extern crate serde;

pub mod tasks;
pub mod user;

pub type Date = String;
pub type DateTimeOffset = String;

/// Contains the `@odata.context` and `@odata.nextLink` properties of a Collection.
/// The latter contains the URL for the next page of results within the Collection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OData {
    #[serde(rename = "@odata.context")]
    pub context: Option<String>,

    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,
}

/// A general collection of results from the Graph API.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Collection<T> {
    pub value: Vec<T>,

    #[serde(flatten)]
    pub odata: OData,
}

/// Represents a `dateTimeTimeZone` resource type.
///
/// @TODO: This could probably be converted into a `chrono` type with
/// a custom deserialization process.
///
/// See: https://docs.microsoft.com/en-us/graph/api/resources/datetimetimezone?view=graph-rest-1.0
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub time_zone: String,
}

/// An attempt at a wrapper for handling either a successful response, or a response with an error.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Response<T> {
    Success(T),

    Error(ErrorResponse)
}

/// An attempt at containing a response error.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: ErrorResponseError,
}


#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseError {
    pub code: String,

    pub message: String,

    inner_error: ErrorResponseErrorInnerError
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseErrorInnerError {
    date: String,

    request_id: String,

    client_request_id: String,
}
