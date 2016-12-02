// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

/*
 * Device representation and control
 */

pub use time::Duration;
pub use url::{Host, Url};
use error::WemoError;
use net::soap::{SoapClient, SoapRequest};
use net::ssdp::{DeviceSearch, SsdpResponse};
use std::fmt::{Display, Error, Formatter};
use std::net::IpAddr;
use std::sync::RwLock;
use super::SerialNumber;
use super::state::WemoState::{Off, On, OnWithoutLoad};
use super::state::WemoState;
use time::PreciseTime;
use url::ParseError;
use xml::find_tag_value;

pub type WemoResult = Result<WemoState, WemoError>;

/// Default Wemo API port (HTTP).
/// Wemo devices change ports occasionally by incrementing the port number.
const DEFAULT_API_PORT: u16 = 49153;

const FIRST_ATTEMPT_TIMEOUT: i64 = 300;

// A method of identifying a WeMo device on the network. When a WeMo device
// goes offline, this is what we use to find it again.
pub enum DeviceIdentifier {
  // A static IP address is the best way to find a device.
  StaticIp(IpAddr),
  // The human-given name of the WeMo device.
  // This is case sensitive and must match exactly.
  // TODO: DeviceName(String),
  // The WeMo serial number unique to the device.
  // TODO: SerialNumber(String),
  // Transient value while this is unimplemented.
  Unimplemented, // TODO: Remove.
}

// TODO: Problems between internalized client, mutability, and clonability

/// Represents a Wemo Switch device.
pub struct Switch {
  /// How we identify the device on the network. A static IP address is optimal.
  device_identifier: DeviceIdentifier,

  /// Location of the device if a dynamic IP address is used.
  dynamic_ip_address: RwLock<Option<IpAddr>>,

  /// Last known port the device used.
  /// Wemo devices are notorious for occasionally changing ports, so we keep
  /// track of the last one we found it using to reduce failed requests and
  /// retries.
  port: RwLock<Option<u16>>,

  // TODO: GET RID OF THIS.
  /// Location of the device in the format `http://ip_address:port`.
  location: Url,

  // TODO: Make private. Only temporary.
  /// The device's unique serial number.
  pub serial_number: Option<SerialNumber>,
}

/// Functions for WeMo Switch.
impl Switch {
  /// Switch CTOR.
  #[deprecated(since="0.0.11")]
  pub fn new(url: Url) -> Switch {
    Switch {
      dynamic_ip_address: RwLock::new(None),
      port: RwLock::new(None),
      device_identifier: DeviceIdentifier::Unimplemented,
      location: url.clone(),
      serial_number: None,
    }
  }

  /// Construct a device that lives behind a static IP address.
  /// We won't need to issue later SSDP searches to find or relocate the device.
  pub fn from_static_ip(ip_address: IpAddr) -> Switch {
    // FIXME: Unsafe code is bad, but this isn't going to stay for long.
    let location = Url::parse(
      &format!("http://{}:{}", ip_address, DEFAULT_API_PORT)).unwrap();
    Switch {
      device_identifier: DeviceIdentifier::StaticIp(ip_address),
      dynamic_ip_address: RwLock::new(None),
      port: RwLock::new(None),
      location: location,
      serial_number: None,
    }
  }

  /// Switch CTOR.
  #[deprecated(since="0.0.11")]
  pub fn from_url(url: &str) -> Result<Switch, ParseError> {
    match Url::parse(url) {
      Ok(parsed_url) => { Ok(Switch::new(parsed_url)) },
      Err(e) => { Err(e) },
    }
  }

  /// Switch CTOR.
  #[deprecated(since="0.0.11")]
  pub fn from_ip_and_port(ip_addr: &str, port: u16) -> Switch {
    // FIXME: No unwrap. This library has bigger problems than this, though.
    let url = Url::parse(&format!("http://{}:{}", ip_addr, port)).unwrap();
    Switch {
      dynamic_ip_address: RwLock::new(None),
      port: RwLock::new(None),
      device_identifier: DeviceIdentifier::Unimplemented,
      location: url.clone(),
      serial_number: None,
    }
  }

  /// Switch CTOR.
  fn from_search_result(search_result: &SsdpResponse) -> Switch {
    // FIXME: Super lame and unsafe.
    let host = search_result.setup_url.host_str().unwrap();
    let port = search_result.port;
    let url = Url::parse(&format!("http://{}:{}", host, port)).unwrap();

    Switch {
      dynamic_ip_address: RwLock::new(Some(search_result.ip_address.clone())),
      port: RwLock::new(None),
      device_identifier: DeviceIdentifier::Unimplemented,
      location: url,
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
    let ip_address = self.get_ip_address().ok_or(WemoError::NoLocalIp)?;

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
    let ip_address = self.get_ip_address().ok_or(WemoError::NoLocalIp)?;

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
  /*pub fn get_ip_address_address(&self) -> Option<Ipv4Addr> {
    self.location.host_str().and_then(|host| {
      match Ipv4Addr::from_str(&host) {
        Err(_) => { None },
        Ok(ip) => { Some(ip) },
      }
    })
  }*/

  /// Returns the static IP if the Wemo was configured with a static IP,
  /// otherwise returns the last cached IP address.
  pub fn get_ip_address(&self) -> Option<IpAddr> {
    match self.device_identifier {
      DeviceIdentifier::StaticIp(ip) => Some(ip.clone()),
      _ => {
        self.dynamic_ip_address.read()
            .ok()
            .and_then(|ip| ip.clone())
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
    let mut url = self.location.clone();
    url.set_path("/setup.xml");
    url
  }

  /// Get the "basic event" URL.
  #[inline]
  pub fn basic_event_url(&self)-> Url {
    let mut url = self.location.clone();
    url.set_path("/upnp/control/basicevent1");
    url
  }
}

impl Display for Switch {
  fn fmt(&self, f : &mut Formatter) -> Result<(), Error> {
    write!(f, "Switch<{}>", self.location)
  }
}

#[cfg(test)]
mod tests {
  use std::net::IpAddr;
  use std::str::FromStr;
  use std::sync::RwLock;
  use super::*;

  #[test]
  fn test_get_ip_address_with_static_ip() {
    let switch = Switch::from_static_ip(IpAddr::from_str("127.0.0.1").unwrap());
    assert_eq!(IpAddr::from_str("127.0.0.1").ok(), switch.get_ip_address());
  }

  #[test]
  fn test_get_ip_address_with_dynamic_ip() {
    let switch = Switch {
      device_identifier: DeviceIdentifier::Unimplemented, // no static IP
      dynamic_ip_address: RwLock::new(IpAddr::from_str("1.1.1.1").ok()),
      port: RwLock::new(None),
      location: Url::parse("http://localhost/").unwrap(),
      serial_number: None,
    };

    assert_eq!(IpAddr::from_str("1.1.1.1").ok(), switch.get_ip_address());

    // If it were to have a static and dynamic IP (not allowed), the static IP
    // is the one that is returned.
    let switch = Switch {
      device_identifier:
          DeviceIdentifier::StaticIp(IpAddr::from_str("2.2.2.2").unwrap()),
      dynamic_ip_address: RwLock::new(IpAddr::from_str("3.3.3.3").ok()),
      port: RwLock::new(None),
      location: Url::parse("http://localhost/").unwrap(),
      serial_number: None,
    };

    assert_eq!(IpAddr::from_str("2.2.2.2").ok(), switch.get_ip_address());
  }

  #[test]
  fn get_get_ip_address_with_no_ip() {
    let switch = Switch {
      device_identifier:
      DeviceIdentifier::Unimplemented,
      dynamic_ip_address: RwLock::new(None),
      port: RwLock::new(None),
      location: Url::parse("http://localhost/").unwrap(),
      serial_number: None,
    };

    assert_eq!(None, switch.get_ip_address());
  }
}
