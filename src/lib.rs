use std::io::{TcpListener, TcpStream};
use std::io::{Acceptor, Listener};
use std::str::from_utf8;
use std::collections::HashMap;

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

#[deriving(PartialEq, PartialOrd)]
pub enum Version {
    HTTP_09,
    HTTP_10,
    HTTP_11,
    HTTP_20
}

pub struct Request {
    pub method: RequestType,
    pub version: Version,
    pub resource: String,
    pub headers: HashMap<String, String>,
    pub body: String
}

// Tokens
pub const CR: u8 = b'\r';
pub const LF: u8 = b'\n';
pub const SP: u8 = b' ';
pub const EOL: &'static [u8] = &[CR, LF];

fn read_request(stream: TcpStream) {
    // Read first line
    // Read header fields
    // Read body
}

#[test]
fn it_works() {
    let tcp_listener = TcpListener::bind("127.0.0.1:3000");
    let mut acceptor = tcp_listener.listen();

    // Spawn HTTP server
    spawn(proc() {
        for mut opt_stream in acceptor.incoming() {
            match opt_stream {
                Err(e) => println!("Error: {}", e),
                Ok(mut stream) => spawn(proc() {
                    loop {
                        let mut buf = [0u8, ..4096];
                        let count = stream.read(&mut buf).unwrap_or(0);

                        if 0 == count {
                            break;
                        }

                        let slice = buf.slice(0, count);
                        let msg = from_utf8(slice).unwrap_or("");

                        println!("server got: {}", msg);

                        stream.write(slice);
                    }
                })
            }
        }
    });

    spawn(proc() {
        let mut stream = TcpStream::connect("127.0.0.1:3000");
        stream.write(b"Hello World!\r\n").unwrap();

        let mut buf = [0u8, ..4096];
        let count = stream.read(&mut buf);
        let msg = from_utf8(&buf).unwrap_or("");
        println!("Client got: {}", msg);
    });
}
