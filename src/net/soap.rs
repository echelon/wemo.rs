// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

use mio::{EventLoop, Handler, Interest, PollOpt, ReadHint, Token};
use mio::tcp::TcpStream;

use std::net::Ipv4Addr;
use std::io::Read;
use std::io::Write;

const CLIENT: Token = Token(0);
const TIMEOUT: Token = Token(1);

/// An HTTP client for making SOAP requests.
pub struct SoapClient {
  stream: TcpStream,
  soap_request: Option<SoapRequest>,
  soap_response: Option<String>,
}


/// Represents a SOAP request to a WeMo device.
#[derive(Clone)]
pub struct SoapRequest {
  pub request_path: String,
  pub soap_action: String,
  pub http_post_payload: String,
}

impl SoapClient {
  pub fn connect(remote_ip_addr: Ipv4Addr, port: u16) -> Option<SoapClient> {
    match TcpStream::connect((remote_ip_addr, port)) {
      Err(_) => { None },
      Ok(stream) => {
        Some(SoapClient {
          stream: stream,
          soap_request: None,
          soap_response: None,
        })
      }
    }
  }

  /// Make a SOAP HTTP request and return the raw response.
  pub fn post(&mut self, soap_request: SoapRequest, timeout_ms: u64) 
      -> Option<String> {
    self.soap_request = Some(soap_request);

    let mut event_loop = EventLoop::new().unwrap();

    event_loop.timeout_ms(TIMEOUT, timeout_ms).unwrap();
    event_loop.register_opt(&self.stream, CLIENT, Interest::writable(),
                            PollOpt::edge()).unwrap();

    event_loop.run(self).unwrap();

    self.soap_response.take()
  }
}

impl Handler for SoapClient {
  type Timeout = Token;
  type Message = ();

  /// Perform the SOAP HTTP request.
  fn writable(&mut self, event_loop: &mut EventLoop<SoapClient>,
              token: Token) {
    if token != CLIENT {
      return;
    }

    let header = {
      let request = match self.soap_request.as_ref() {
        Some(req) => { req },
        None => { 
          return; 
        },
      };

      format!("\
          POST {} HTTP/1.1\r\n\
          Content-Type: text/xml; charset=\"utf-8\"\r\n\
          Accept:\r\n\
          SOAPACTION: \"{}\"\r\n\
          Content-Length: {}\r\n\
          \r\n\
          {}",
          &request.request_path,
          &request.soap_action,
          &request.http_post_payload.len(),
          &request.http_post_payload)
    };

    match self.stream.write_all(&mut header.as_bytes()) {
      Err(_) => {},
      Ok(_) => {},
    }

    event_loop.deregister(&self.stream).unwrap();
    event_loop.register_opt(&self.stream, CLIENT, Interest::readable(),
                            PollOpt::edge()).unwrap();

    self.soap_request = None;
  }

  /// Read and save the HTTP response.
  fn readable(&mut self, event_loop: &mut EventLoop<SoapClient>,
              token: Token, _: ReadHint) {
    if token != CLIENT {
      return;
    }

    // FIXME: Better buffering.
    let mut buf: Vec<u8> = Vec::with_capacity(1024 * 10);
    let result = self.stream.read_to_end(&mut buf);

    match result {
      Err(_) => {},
      Ok(_) => {
        let response = String::from_utf8(buf).unwrap();
        self.soap_response = Some(response);
      },
    }

    event_loop.shutdown();
  }

  /// Timeout the SOAP HTTP request.
  fn timeout(&mut self, event_loop: &mut EventLoop<SoapClient>,
             token: Token) {
    match token {
      TIMEOUT => { event_loop.shutdown(); },
      _ => {},
    }
  }
}

