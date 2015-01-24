use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

pub mod parser;

#[deriving(Show)]
pub enum Error {
    MethodParseError,
    ResourceParseError,
    VersionParseError,
    MalformedHeaderLineError,
    BodyParsingError,
    StatusCodeParseError,
    StatusReasonParseError
}

#[deriving(Show, PartialEq)]
pub enum RequestType {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    TRACE,
    OPTIONS,
    CONNECT,
    PATCH
}

#[deriving(Show,PartialEq,Clone)]
pub enum HeaderVal {
    List(Vec<String>),
    Val(String),
    None
}

impl HeaderVal {
    fn to_string(self) -> String {
        match self {
            HeaderVal::List(list) => list.iter().fold(String::new(), |mut acc, x| { acc.push_str(x.to_string().as_slice()); return acc; } ),
            HeaderVal::Val(s) => s,
            HeaderVal::None => String::new()
        }
    }
}



#[deriving(PartialEq, PartialOrd, Show)]
pub enum Version {
    Http09,
    Http10,
    Http11,
    Http20
}

#[deriving(Show)]
pub struct Request {
    pub method: RequestType,
    pub version: Version,
    pub resource: String,
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}

#[deriving(Show)]
pub struct Response {
    pub version: Version,
    pub status_code: isize,
    pub reason: String,
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}
