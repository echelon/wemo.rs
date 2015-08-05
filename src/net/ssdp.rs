// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

extern crate mio;

use mio::{EventLoop, Handler, Interest, PollOpt, ReadHint, Token};
use mio::udp::UdpSocket;

use regex::Regex;
use url::Url;

use std::collections::HashMap;
use std::net::{AddrParseError, Ipv4Addr};
use std::str::FromStr;

use device::SerialNumber;

/// Within a given search request, resend SSDP search requests
/// every n millisec (until search request timeout).
const RESEND_SSDP_MS: u64 = 300;

const UPNP_PORT: u16 = 1900;
const LISTENER: Token = Token(0);
const SENDER: Token = Token(1);
const TIMER_RESEND_SSDP: Token = Token(3);
const TIMER_TIMEOUT: Token = Token(4);

/// WeMo Device SSDP Responses.
#[derive(Clone,Debug)]
pub struct SsdpResponse {
  pub serial_number: SerialNumber,
  pub ip_address: Ipv4Addr,
  pub port: u16,
  pub setup_url: Url,
}

/// Uses UPNP SSDP to discover WeMo devices on the local network.
pub struct DeviceSearch {
  /// All of the found devices. Persisted between search requests.
  found_devices: HashMap<SerialNumber, SsdpResponse>,

  /// If present, search will end as soon as the device is found.
  target_serial: Option<SerialNumber>,

  /// If present, search will end as soon as the device is found.
  target_ip_address: Option<Ipv4Addr>,

  /// Socket for SSDP search.
  socket: UdpSocket,
}

impl DeviceSearch {

  /// DeviceSearch CTOR.
  pub fn new() -> DeviceSearch {
    DeviceSearch {
      found_devices: HashMap::new(),
      target_serial: None,
      target_ip_address: None,
      socket: UdpSocket::bind((Ipv4Addr::new(0, 0, 0, 0), 0)).unwrap(),
    }
  }

  /// Search for all devices on the network.
  pub fn search(&mut self, timeout_ms: u64)
      -> &HashMap<SerialNumber, SsdpResponse> {
    //println!("search");
    let mut event_loop = EventLoop::new().unwrap();
    event_loop.register_opt(&self.socket, SENDER, Interest::writable(),
                            PollOpt::edge()).unwrap();

    event_loop.timeout_ms(TIMER_RESEND_SSDP, RESEND_SSDP_MS).unwrap();
    event_loop.timeout_ms(TIMER_TIMEOUT, timeout_ms).unwrap();

    event_loop.run(self).unwrap();

    &self.found_devices
  }

  /// Search for a particular device by serial number.
  /// Exits early when the target device is found.
  pub fn search_for_serial(&mut self, target: &SerialNumber, timeout_ms: u64)
      -> Option<&SsdpResponse> {
    self.target_serial = Some(target.to_string());
    self.search(timeout_ms);
    self.found_devices.get(target)
  }

  /// Search for a particular device by IP address.
  /// Exits early when the target device is found.
  pub fn search_for_ip(&mut self, target: &Ipv4Addr, timeout_ms: u64)
      -> Option<&SsdpResponse> {
    self.target_ip_address = Some(target.clone());
    self.search(timeout_ms);

    for result in self.found_devices.values() {
      if &result.ip_address == target {
        return Some(result);
      }
    }
    None
  }

  /// Whether search results were found.
  pub fn has_results(&self) -> bool {
    self.found_devices.len() != 0
  }

  /// Get the results.
  pub fn get_results(&self) -> &HashMap<SerialNumber, SsdpResponse> {
    &self.found_devices
  }

  /// Reset the search results and search target, if set.
  pub fn reset(&mut self) {
    self.found_devices = HashMap::new();
    self.target_serial = None;
    self.target_ip_address = None;
  }
}

impl Handler for DeviceSearch {
  type Timeout = Token;
  type Message = u32;

  /// Send SSDP search command.
  fn writable(&mut self, event_loop: &mut EventLoop<DeviceSearch>,
              token: Token) {
    let multicast_ip = Ipv4Addr::new(239, 255, 255, 250);

    // "ST:upnp:rootdevice\r\n" // All SSDP/UPNP hardware.
    // "ST:urn:Belkin:device:lightswitch:1\r\n" // Lightswitch.

    let header = format!("\
        M-SEARCH * HTTP/1.1\r\n\
        HOST: {}:{}\r\n\
        ST:urn:Belkin:device:*\r\n\
        MAN:\"ssdp:discover\"\r\n\
        MX:5\r\n\
        \r\n",
        &multicast_ip,
        &UPNP_PORT);

    match token {
      SENDER => {
        self.socket.send_to(&mut header.as_bytes(), (multicast_ip, UPNP_PORT))
            .unwrap();
      },
      _ => (),
    };

    event_loop.reregister(&self.socket, LISTENER, Interest::readable(),
                          PollOpt::edge()).unwrap();
  }

  /// Read SSDP responses and add WeMo devices to the map.
  fn readable(&mut self, event_loop: &mut EventLoop<DeviceSearch>,
              token: Token, _: ReadHint) {
    // FIXME: Cleanup this awful garbage code.
    let mut buf = [0; 1024 * 1024];

    let parsed_response = match token {
      LISTENER => {
        let result = self.socket.recv_from(&mut buf);
        match result {
          Err(_) => { None },
          Ok((amt, _)) => {
            let mut vec: Vec<u8> = Vec::with_capacity(amt);
            for i in 0 .. amt {
              vec.push(buf[i]);
            }

            let response_headers = String::from_utf8(vec).unwrap();
            parse_search_result(response_headers.as_ref())
          },
        }
      },
      _ => None,
    };

    if parsed_response.is_some() {
      let device = parsed_response.unwrap();
      let serial_number = device.serial_number.clone();
      let ip_address: Ipv4Addr = device.ip_address.clone();

      self.found_devices.insert(serial_number.clone(), device);

      if self.target_serial.is_some() {
        let cmp: &str = serial_number.as_ref();

        if self.target_serial.as_ref().unwrap() == cmp {
          event_loop.shutdown();
          return;
        }
      } else if self.target_ip_address.is_some() {
        if self.target_ip_address.as_ref().unwrap() == &ip_address {
          event_loop.shutdown();
          return;
        }
      }
    }
  }

  /// Manages timeouts: reenqueuing search and overall search timeout.
  fn timeout(&mut self, event_loop: &mut EventLoop<DeviceSearch>,
             token: Token) {
    match token {
      TIMER_TIMEOUT => { event_loop.shutdown(); },
      TIMER_RESEND_SSDP => {
        // Resend the SSDP search request every `RESEND_SSDP_MS` as long
        // as we're still searching (eg. TIMER_TIMEOUT not called).
        event_loop.reregister(&self.socket, SENDER, Interest::writable(),
                          PollOpt::edge()).unwrap();
        event_loop.timeout_ms(TIMER_RESEND_SSDP, RESEND_SSDP_MS).unwrap();
      },
      _ => {},
    }
  }
}

/// Parse the WeMo SSDP Response Headers.
/// The location header, `LOCATION: http://192.168.1.4:49153/setup.xml`,
/// becomes `http://192.168.1.4:49153/setup.xml`.
/// The USN header, `USN: uuid:Insight-1_0-12345ABCDE::upnp:rootdevice`,
/// contains the serial number `12345ABCDE`.
fn parse_search_result(response_headers: &str) -> Option<SsdpResponse> {
  // FIXME: Cleanup parsing code.
  let location_regex = Regex::new(r"(?im:^LOCATION:\s*(.*)$)").unwrap();
  let serial_regex = Regex::new(
      r"(?im:^USN:\s*uuid:(Lightswitch|Insight|Socket)-\d_\d-(.*)::)")
          .unwrap();

  let url_result : Option<Url> = {
    let mut result : Option<Url> = None;
    for cap in location_regex.captures_iter(response_headers) {
      let matched_url = cap.at(1).unwrap_or("");
      result = match Url::parse(matched_url) {
        Ok(u) => { Some(u) },
        Err(_) => { None },
      }
    }
    result
  };

  if url_result.is_none() { return None; }

  let url = url_result.unwrap();

  if url.host().is_none() { return None; }

  let host = url.host().unwrap();
  let port = url.port_or_default().unwrap_or(80);

  let ip_address : Result<Ipv4Addr, AddrParseError>
      = Ipv4Addr::from_str(host.serialize().as_ref());

  if ip_address.is_err() { return None; }

  let serial_number : Option<SerialNumber> = {
    let mut result : Option<SerialNumber> = None;
    for cap in serial_regex.captures_iter(response_headers) {
      let parsed = cap.at(2).unwrap_or("");
      result = Some(parsed.to_string());
    }
    result
  };

  if serial_number.is_none() { return None; }

  Some(SsdpResponse {
    serial_number: serial_number.unwrap(),
    ip_address: ip_address.unwrap(),
    port: port,
    setup_url: url.clone(),
  })
}

