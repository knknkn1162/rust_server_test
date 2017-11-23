#[macro_use]
extern crate log;
extern crate env_logger;

extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;
extern crate bytes;
extern crate futures;

use std::{io, str};
use std::io::Read;
use std::path::PathBuf;
use bytes::{BufMut, BytesMut};

use futures::{future, Future};

use tokio_io::codec::{Encoder, Decoder, Framed};
use tokio_io::{AsyncRead, AsyncWrite};

use tokio_proto::pipeline::{ServerProto};
use tokio_proto::{TcpServer};
use tokio_service::{Service, NewService};

#[derive(Debug)]
pub struct Request {
    pub uri: String,
}

#[derive(Debug)]
pub struct Response {
    body: String,
}

impl Response {
    pub fn with_body<B: Into<String>>(body: B)->Response {
        Response {body: body.into()}
    }
}

pub struct HttpCodec;

impl Decoder for HttpCodec {
    type Item = Request;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut)-> io::Result<Option<Self::Item>> {
        if let Some(i) =  buf.iter().position(|&b| b == b'\r') {
            //debug!("{:?}", buf);
            let line = buf.split_to(i);

            // remove '\r\n';
            buf.split_to(2);

            debug!("    {} call decode({:?}) => complete", if line!=&""[..] {"<"} else {" "}, line);
            return if &line[0..4] == &b"GET "[..] {
                match str::from_utf8(&line[4..]) {
                    Ok(s) => Ok(Some(Request { uri: s.into() })),
                    Err(e) => Err(io::Error::new(io::ErrorKind::Other, e.to_string())),
                }
            } else {
                Err(io::Error::new(io::ErrorKind::Other, "invalid protocol"))
            }
        }
        Ok(None)
    }
}

impl Encoder for HttpCodec {
    type Item = Response;
    type Error = io::Error;

    fn encode(&mut self, res: Self::Item, buf: &mut BytesMut)-> io::Result<()> {
        // use bytes::BufMut;
        buf.put(res.body.as_bytes());
        buf.put(&b"\r\n"[..]);

        debug!("    {} call encode => complete({:?})", if buf!=&""[..] {">"} else {" "}, buf);
        Ok(())
    }
}

pub struct Http;

impl<T: AsyncRead + AsyncWrite + 'static> ServerProto<T> for Http {
    type Request = Request;
    type Response = Response;
    type Transport = Framed<T, HttpCodec>;
    type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T)-> Self::BindTransport {
        debug!("<Http as ServerProto>::bind_transport");
        Ok(io.framed(HttpCodec))
    }
}

pub struct FileService {
    root: PathBuf,
}

impl Service for FileService {
    type Request = Request;
    type Response = Response;

    type Error = io::Error;
    type Future = future::FutureResult<Self::Response, Self::Error>;

    fn call(&self, req: Self::Request)-> Self::Future {
        debug!("<FileService as Service>::call({:?})", req);
        let path = self.root.join(&req.uri[1..]);

        if path.is_file() {
            let mut body = String::new();
            match ::std::fs::File::open(path).and_then(|mut f| f.read_to_string(&mut body)) {
                Ok(_) => future::ok(Response::with_body(body)),
                Err(err) => future::err(err),
            }
        } else {
            future::err(io::Error::new(io::ErrorKind::Other, "not found"))
        }
    }
}

pub struct FileServer {
    root: PathBuf,
}

impl FileServer {
    pub fn new<P: Into<PathBuf>>(root: P)-> Self {
        FileServer{root: root.into()}
    }
}

impl NewService for FileServer {
    type Request = Request;
    type Response = Response;
    type Error = io::Error;
    type Instance = FileService;

    fn new_service(&self)-> io::Result<Self::Instance> {
        debug!("start new_service");
        Ok(FileService{root: self.root.clone()})
    }
}

/// RUST_LOG=simple_line=debug cargo run --bin simple_line
fn main() {
    env_logger::init().unwrap();
    let addr = "127.0.0.1:8081".parse().unwrap();

    let mut server = TcpServer::new(Http, addr);

    debug!("start server");

    // server side
    /*
    DEBUG:simple_line: start server
    DEBUG:simple_line: start new_service
    DEBUG:simple_line: <Http as ServerProto>::bind_transport
    DEBUG:simple_line:     < call decode(b"GET\x20/index.html") => complete
    DEBUG:simple_line: <FileService as Service>::call(Request { uri: "/index.html" })
    DEBUG:simple_line: b""
    DEBUG:simple_line:     > call encode => complete(b"<!DOCTYPE\x20html>\n<html\x20lang=\"en\">\n<head>\n\x20\x20\x20\x20<meta\x20charset=\"UTF-8\">\n\x20\x20\x20\x20<title>Sample\x20Title</title>\n</head>\n<body>\n\x20\x20\x20\x20<p>My\x20first\x20tokio-proto!</p>\n</body>\n</html>\r\n")
    */
    server.serve(FileServer::new("static/"));

    // client side
    /*
    $ telnet localhost 8081
    Trying ::1...
    telnet: connect to address ::1: Connection refused
    Trying 127.0.0.1...
    Connected to localhost.
    Escape character is '^]'.
    GET /index.html
    <!DOCTYPE html>
    <html lang="en">
    <head>
        <meta charset="UTF-8">
        <title>Sample Title</title>
    </head>
    <body>
        <p>My first tokio-proto!</p>
    </body>
    </html>
    */
}