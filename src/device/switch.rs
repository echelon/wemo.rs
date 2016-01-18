// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

/*
 * Device representation and control
 */

pub use time::Duration;
pub use url::{Host, Url, SchemeData, RelativeSchemeData};

use std::net::Ipv4Addr;
use std::str::FromStr;
use std::fmt::{Display, Error, Formatter};

use time::PreciseTime;
use url::{ParseError, UrlParser};
use xml::find_tag_value;

use super::SerialNumber;
use super::error::WemoError;
use super::state::WemoState::{Off, On, OnWithoutLoad};
use super::state::WemoState;
use net::soap::{SoapClient, SoapRequest};
use net::ssdp::{DeviceSearch, SsdpResponse};

pub type WemoResult = Result<WemoState, WemoError>;

const FIRST_ATTEMPT_TIMEOUT: i64 = 300;

// TODO: Problems between internalized client, mutability, and clonability

/// Represents a Wemo Switch device.
pub struct Switch {
  /// Location of the device in the format `http://ip_address:port`.
  location: Url,

  // TODO: Make private. Only temporary.
  /// The device's unique serial number.
  pub serial_number: Option<SerialNumber>,
}

/// Functions for WeMo Switch.
impl Switch {
  /// Switch CTOR.
  #[inline]
  pub fn new(url: Url) -> Switch {
    Switch {
      location: url.clone(),
      serial_number: None,
    }
  }

  /// Switch CTOR.
  #[inline]
  pub fn from_url(url: &str) -> Result<Switch, ParseError> {
    match Url::parse(url) {
      Ok(parsed_url) => { Ok(Switch::new(parsed_url)) },
      Err(e) => { Err(e) },
    }
  }

  /// Switch CTOR.
  #[inline]
  pub fn from_ip_and_port(ip_addr: &str, port: u16) -> Switch {
    Switch::new(
      Url {
        scheme_data: SchemeData::Relative(RelativeSchemeData {
          host: Host::parse(ip_addr).unwrap(),
          port: Some(port),
          default_port: None,
          password: None,
          path: Vec::new(),
          username: "".to_string(),
        }),
        scheme: "http".to_string(),
        query: None,
        fragment: None,
      }
    )
  }

  /// Switch CTOR.
  #[inline]
  fn from_search_result(search_result: &SsdpResponse) -> Switch {
    // TODO: Unwrap Safety.
    let host = search_result.setup_url.host().unwrap().serialize();

    Switch {
      location: Url {
        scheme_data: SchemeData::Relative(RelativeSchemeData {
          host: Host::parse(&host).unwrap(),
          port: Some(search_result.port),
          default_port: None,
          password: None,
          path: Vec::new(),
          username: "".to_string(),
        }),
        scheme: "http".to_string(),
        query: None,
        fragment: None,
      },
      serial_number: Some(search_result.serial_number.clone()),
    }
  }

  /// Turn the device on.
  pub fn turn_on(&self, timeout: Duration) -> WemoResult {
    info!(target: "wemo", "Turning on: {}", self.location);
    self.set_state(On, timeout)
  }

  /// Turn the device on.
  pub fn turn_on_with_retry(&self, timeout: Duration) -> WemoResult {
    info!(target: "wemo", "Turning on with retry: {}", self.location);
    self.set_state_with_retry(On, timeout)
  }

  /// Turn the device off.
  pub fn turn_off(&self, timeout: Duration) -> WemoResult {
    info!(target: "wemo", "Turning off: {}", self.location);
    self.set_state(Off, timeout)
  }

  /// Turn the device off.
  pub fn turn_off_with_retry(&self, timeout: Duration) -> WemoResult {
    info!(target: "wemo", "Turning off with retry: {}", self.location);
    self.set_state_with_retry(Off, timeout)
  }

  /// Toggle the device on or off.
  pub fn toggle(&self, timeout: Duration) -> WemoResult {
    let mut state: Option<WemoState> = None;
    let mut error: Option<WemoError> = None;

    let elapsed = Duration::span(|| {
      match self.get_state(timeout) {
        Ok(result) => {
          state = Some(result);
        },
        Err(_) => {
          error = Some(WemoError::BadResponseError); // TODO: Wrong error
        },
      }
    });

    if error.is_some() {
      return Err(error.unwrap());
    } else if elapsed > timeout {
      return Err(WemoError::TimeoutError);
    }

    let remaining = timeout - elapsed;

    match state {
      Some(Off) => {
        self.turn_on(remaining)
      },
      Some(On) => {
        self.turn_off(remaining)
      },
      Some(OnWithoutLoad) => {
        self.turn_off(remaining)
      },
      Some(_) | None => {
        Err(WemoError::WemoError)
      },
    }
  }

  /// Toggle the device on or off.
  pub fn toggle_with_retry(&self, timeout: Duration) -> WemoResult {
    let mut state: Option<WemoState> = None;
    let mut error: Option<WemoError> = None;

    let elapsed = Duration::span(|| {
      match self.get_state_with_retry(timeout) {
        Ok(result) => {
          state = Some(result);
        },
        Err(_) => {
          error = Some(WemoError::BadResponseError); // TODO: Wrong error
        },
      }
    });

    if error.is_some() {
      return Err(error.unwrap());
    } else if elapsed > timeout {
      return Err(WemoError::TimeoutError);
    }

    let remaining = timeout - elapsed;

    match state {
      Some(Off) => {
        self.turn_on_with_retry(remaining)
      },
      Some(On) => {
        self.turn_off_with_retry(remaining)
      },
      Some(OnWithoutLoad) => {
        self.turn_off_with_retry(remaining)
      },
      Some(_) | None => {
        Err(WemoError::WemoError)
      },
    }
  }

  /// Get the current state of the device.
  pub fn get_state(&self, timeout: Duration) -> WemoResult {
    let ip_address = match self.get_ip_address() {
      Some(ip) => { ip },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let port = match self.get_port() {
      Some(port) => { port },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let mut client = match SoapClient::connect(ip_address, port) {
      Some(c) => { c },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let xml_body = "\
      <?xml version=\"1.0\" encoding=\"utf-8\"?>\
        <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"\
            s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
          <s:Body>\
            <u:GetBinaryState xmlns:u=\"urn:Belkin:service:basicevent:1\">\
              <BinaryState>1</BinaryState>\
            </u:GetBinaryState>\
          </s:Body>\
        </s:Envelope>";

    let request = SoapRequest {
      request_path: "/upnp/control/basicevent1".to_string(),
      soap_action: "urn:Belkin:service:basicevent:1#GetBinaryState".to_string(),
      http_post_payload: xml_body.to_string(),
    };

    let response = client.post(request, timeout.num_milliseconds() as u64);

    // TODO: Stronger return error types
    let body = match response {
      Some(r) => { r },
      None => {
        return Err(WemoError::BadResponseError);
      }
    };

    // TODO: Error handle.
    let state = find_tag_value("BinaryState", body.as_ref()).unwrap_or("");
    match WemoState::from_i64(state.parse::<i64>().unwrap()) {
      Some(result) => {
        Ok(result)
      },
      None => {
        Err(WemoError::WemoError)
      }
    }
  }

  /// Set the current state of the device.
  pub fn set_state(&self, state: WemoState, timeout: Duration) -> WemoResult {
    let ip_address = match self.get_ip_address() {
      Some(ip) => { ip },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let port = match self.get_port() {
      Some(port) => { port },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let mut client = match SoapClient::connect(ip_address, port) {
      Some(c) => { c },
      None => {
        return Err(WemoError::BadResponseError); // TODO WRONG TYPE
      },
    };

    let xml_body = format!("\
      <?xml version=\"1.0\" encoding=\"utf-8\"?>\
        <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"\
            s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
          <s:Body>\
            <u:SetBinaryState xmlns:u=\"urn:Belkin:service:basicevent:1\">\
              <BinaryState>{}</BinaryState>\
            </u:SetBinaryState>\
          </s:Body>\
        </s:Envelope>\
      ", state.to_i8());

    let request = SoapRequest {
      request_path: "/upnp/control/basicevent1".to_string(),
      soap_action: "urn:Belkin:service:basicevent:1#SetBinaryState".to_string(),
      http_post_payload: xml_body.to_string(),
    };

    let response = client.post(request, timeout.num_milliseconds() as u64);

    match response {
      Some(_) => { Ok(state)  }, // TODO: Check to ensure matches requested state
      None => { Err(WemoError::BadResponseError) },
    }
  }

  // TODO: Make private.
  pub fn get_state_with_retry(&self, timeout: Duration) -> WemoResult {
    let mut start = PreciseTime::now();

    // TODO: use the minimum of the timestamps
    let result = self.get_state(Duration::milliseconds(FIRST_ATTEMPT_TIMEOUT));

    match result {
      Ok(r) => { return Ok(r); },
      Err(_) => {}, // TODO
    }

    let mut elapsed = start.to(PreciseTime::now());

    if elapsed > timeout {
      return Err(WemoError::TimeoutError);
    }

    let mut remaining = timeout - elapsed;
    if remaining <= Duration::zero() {
      return Err(WemoError::TimeoutError);
    }

    start = PreciseTime::now();

    let switch = match self.relocate(remaining) {
      None => { return Err(WemoError::TimeoutError); }, // TODO: Wrong.
      Some(s) => { s },
    };

    elapsed = start.to(PreciseTime::now());
    if elapsed > remaining {
      return Err(WemoError::TimeoutError);
    }

    remaining = remaining - elapsed;
    if remaining <= Duration::zero() {
      return Err(WemoError::TimeoutError);
    }

    switch.get_state(remaining)
  }

  // TODO: Make private
  pub fn set_state_with_retry(&self, state: WemoState, timeout: Duration)
      -> WemoResult {
    let mut start = PreciseTime::now();

    // TODO: use the minimum of the timestamps
    let result = self.set_state(state.clone(),
        Duration::milliseconds(FIRST_ATTEMPT_TIMEOUT));

    match result {
      Ok(r) => { return Ok(r); },
      Err(_) => {}, // TODO: Return type
    }

    let mut elapsed = start.to(PreciseTime::now());

    if elapsed > timeout {
      return Err(WemoError::TimeoutError);
    }

    let mut remaining = timeout - elapsed;
    if remaining <= Duration::zero() {
      return Err(WemoError::TimeoutError);
    }

    start = PreciseTime::now();

    let switch = match self.relocate(remaining) {
      None => {
        return Err(WemoError::TimeoutError); // TODO: Wrong err.
      },
      Some(s) => { s },
    };

    elapsed = start.to(PreciseTime::now());
    if elapsed > remaining {
      return Err(WemoError::TimeoutError);
    }

    remaining = remaining - elapsed;
    if remaining <= Duration::zero() {
      return Err(WemoError::TimeoutError);
    }

    switch.set_state(state.clone(), remaining)
  }

  /// Get the currently known IP address.
  pub fn get_ip_address(&self) -> Option<Ipv4Addr> {
    match self.location.serialize_host() {
      None => { None },
      Some(host) => {
        match Ipv4Addr::from_str(&host) {
          Err(_) => { None },
          Ok(ip) => { Some(ip) },
        }
      },
    }
  }

  /// Get the currently known port.
  #[inline]
  pub fn get_port(&self) -> Option<u16> {
    self.location.port()
  }

  /// Attempt to find the Switch on the network via SSDP.
  pub fn relocate(&self, timeout: Duration) -> Option<Switch> {
    if self.serial_number.is_some() {
      // Guaranteed to be the same device unless there is spoofing
      // (or Belkin assigned duplicate serial numbers).
      self.relocate_by_serial(timeout)
    } else {
      // Won't necessarily be the same device if DHCP has reassigned
      // the address.
      self.relocate_by_ip(timeout)
    }
  }

  fn relocate_by_serial(&self, timeout: Duration) -> Option<Switch> {
    let serial = match self.serial_number {
      None => { return None; },
      Some(ref s) => { s },
    };

    let mut search = DeviceSearch::new();

    match search.search_for_serial(serial, timeout.num_milliseconds() as u64){
      None => { None },
      Some(result) => { Some(Switch::from_search_result(result)) },
    }
  }

  fn relocate_by_ip(&self, timeout: Duration) -> Option<Switch> {
    let ip_address = match self.get_ip_address() {
      None => { return None; },
      Some(ip) => { ip },
    };

    let mut search = DeviceSearch::new();

    match search.search_for_ip(&ip_address, timeout.num_milliseconds() as u64) {
      None => { None },
      Some(result) => { Some(Switch::from_search_result(result)) },
    }
  }

  /// Get the base URL.
  #[inline]
  pub fn base_url(&self) -> &Url {
    &self.location
  }

  /// Get the "setup"/info URL.
  #[inline]
  pub fn setup_url(&self) -> Url {
    UrlParser::new().base_url(&self.location)
      .parse("/setup.xml")
      .unwrap()
  }

  /// Get the "basic event" URL.
  #[inline]
  pub fn basic_event_url(&self)-> Url {
    UrlParser::new().base_url(&self.location)
      .parse("/upnp/control/basicevent1")
      .unwrap()
  }
}

impl Display for Switch {
  fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
    write!(f, "Switch<{}>", self.location)
  }
}
