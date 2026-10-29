use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bumped on any incompatible change to method names, shapes or events.
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

impl Response {
    pub fn ok(id: u64, result: Value) -> Self {
        Response { id, result: Some(result), error: None }
    }
    pub fn err(id: u64, code: &str, message: impl Into<String>) -> Self {
        Response {
            id,
            result: None,
            error: Some(ErrorBody { code: code.into(), message: message.into() }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub event: String,
    pub data: Value,
}

/// What a client may send. Only requests today; the enum leaves room for a `cancel`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Incoming {
    Request(Request),
}

/// What the daemon sends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Outgoing {
    Response(Response),
    Event(Event),
}

impl Outgoing {
    /// One line, newline included.
    pub fn to_line(&self) -> String {
        let mut s = serde_json::to_string(self).expect("wire types serialize");
        s.push('\n');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_round_trips_with_camel_case_params() {
        let line = r#"{"id":7,"method":"play","params":{"item":{"videoId":"abc"}}}"#;
        let req: Incoming = serde_json::from_str(line).unwrap();
        let Incoming::Request(req) = req;
        assert_eq!(req.id, 7);
        assert_eq!(req.method, "play");
        assert_eq!(req.params["item"]["videoId"], "abc");
    }

    #[test]
    fn params_default_to_null() {
        let Incoming::Request(req) = serde_json::from_str(r#"{"id":1,"method":"hello"}"#).unwrap();
        assert!(req.params.is_null());
    }

    #[test]
    fn response_omits_the_absent_half() {
        assert_eq!(
            Outgoing::Response(Response::ok(1, Value::Bool(true))).to_line(),
            "{\"id\":1,\"result\":true}\n"
        );
        assert_eq!(
            Outgoing::Response(Response::err(2, "upload_unavailable", "x")).to_line(),
            "{\"id\":2,\"error\":{\"code\":\"upload_unavailable\",\"message\":\"x\"}}\n"
        );
    }

    #[test]
    fn events_and_responses_are_distinguishable_on_the_wire() {
        let ev: Outgoing =
            serde_json::from_str(r#"{"event":"position","data":{"position":1.5}}"#).unwrap();
        assert!(matches!(ev, Outgoing::Event(_)));
        let resp: Outgoing = serde_json::from_str(r#"{"id":3,"result":null}"#).unwrap();
        assert!(matches!(resp, Outgoing::Response(_)));
    }

    #[test]
    fn error_codes_and_messages_survive_the_wire_round_trip() {
        for (code, message) in [
            ("bad_params", "channelId: missing field `channelId`"),
            ("failed", "Sign in first to use this."),
            ("spotify_signed_out", "Sign in to Spotify"),
            ("client_side", "handled on the client"),
        ] {
            let line = Outgoing::Response(Response::err(4, code, message)).to_line();
            let back: Outgoing = serde_json::from_str(&line).unwrap();
            let Outgoing::Response(resp) = back else {
                panic!("error response did not parse as a response")
            };
            let err = resp.error.expect("error half present");
            assert_eq!((err.code.as_str(), err.message.as_str()), (code, message));
            assert!(resp.result.is_none());
        }
    }

    #[test]
    fn responses_carry_either_a_result_or_an_error_never_both() {
        let ok_line = Outgoing::Response(Response::ok(1, serde_json::json!(true))).to_line();
        assert!(!ok_line.contains("error"));
        let err_line = Outgoing::Response(Response::err(1, "failed", "x")).to_line();
        assert!(!err_line.contains("result"));
    }

    #[test]
    fn request_ids_are_preserved_for_response_correlation() {
        for id in [0u64, 1, 42, u64::MAX] {
            let line = Outgoing::Response(Response::ok(id, serde_json::Value::Null)).to_line();
            let Outgoing::Response(resp) = serde_json::from_str(&line).unwrap() else {
                panic!("response did not parse as a response")
            };
            assert_eq!(resp.id, id);
        }
    }

    #[test]
    fn events_carry_scalar_object_string_and_null_payloads() {
        for data in [
            serde_json::json!(1.5),
            serde_json::json!({ "position": 1.5 }),
            serde_json::json!("plain notice"),
            serde_json::Value::Null,
        ] {
            let line =
                Outgoing::Event(Event { event: "position".into(), data: data.clone() }).to_line();
            let Outgoing::Event(ev) = serde_json::from_str(&line).unwrap() else {
                panic!("event did not parse as an event")
            };
            assert_eq!(ev.data, data);
        }
    }

    #[test]
    fn requests_with_absent_params_still_parse() {
        let req = match serde_json::from_str::<Incoming>(r#"{"id":2,"method":"hello"}"#).unwrap() {
            Incoming::Request(req) => req,
        };
        assert!(req.params.is_null());
        let req = match serde_json::from_str::<Incoming>(r#"{"id":2,"method":"hello","params":{}}"#)
            .unwrap()
        {
            Incoming::Request(req) => req,
        };
        assert!(req.params.as_object().unwrap().is_empty());
    }
}
