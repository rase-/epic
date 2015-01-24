use std::io::IoResult;
use std::str::from_utf8;
use std::collections::HashMap;

use std::fmt;
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;

pub mod parser;

#[derive(Debug)]
pub enum HTTPError {
    MethodParseError,
    ResourceParseError,
    VersionParseError,
    MalformedHeaderLineError,
    BodyParsingError,
    StatusCodeParseError,
    StatusReasonParseError
}

impl Error for HTTPError {
    fn description(&self) -> &str {
        match *self {
           HTTPError::MethodParseError => "MethodParseError",
           HTTPError::ResourceParseError => "ResourceParseError" ,
           HTTPError::VersionParseError => "VersionParseError",
           HTTPError::MalformedHeaderLineError => "MalformedHeaderLineError",
           HTTPError::BodyParsingError => "BodyParsingError",
           HTTPError::StatusCodeParseError => "StatusCodeParseError",
           HTTPError::StatusReasonParseError => "StatusReasonParseError"
        }
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

impl Display for HTTPError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.description().fmt(f)
    }
}

#[derive(Debug, PartialEq, Clone)]
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

#[derive(Debug,PartialEq,Clone)]
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



#[derive(PartialEq, PartialOrd, Debug)]
pub enum Version {
    Http09,
    Http10,
    Http11,
    Http20
}

#[derive(Debug)]
pub struct Request {
    pub method: RequestType,
    pub version: Version,
    pub resource: String,
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}

#[derive(Debug)]
pub struct Response {
    pub version: Version,
    pub status_code: isize,
    pub reason: String,
    pub headers: HashMap<String, HeaderVal>,
    pub body: Option<String>
}
