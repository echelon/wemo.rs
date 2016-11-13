// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>

//! This module abstracts and hides away all of the bad parsing behavior of
//! the library. Since there is no lightweight, well-vetted XML library yet, I
//! am committing one of the gravest of sins in order to parse results from
//! responses: using regular expressions. Please don't hate me.

use device::state::WemoState;
use error::WemoError;
use regex::Regex;

/// Parse the device state from XML returned via subscription events.
pub fn parse_state(xml: &str) -> Result<WemoState, WemoError> {
  lazy_static! {
    static ref RE: Regex =
        Regex::new(r"<BinaryState>(\d)(\|\d+)*</BinaryState>").unwrap();
  }

  let matches = RE.captures(xml).ok_or(WemoError::ParsingError)?;
  let state = matches.at(1).ok_or(WemoError::ParsingError)?;

  match state {
    "0" => Ok(WemoState::Off),
    "1" => Ok(WemoState::On),
    "8" => Ok(WemoState::OnWithoutLoad),
    _ => Err(WemoError::ParsingError), // TODO: Drop "unknown" WemoState.
  }
}

#[cfg(test)]
mod tests {
  use device::state::WemoState;
  use error::WemoError;
  use super::*;

  #[test]
  fn switch_notifications() {
    let xml = r#"
      <e:propertyset xmlns:e="\#urn:schemas-upnp-org:event-1-0">
        <e:property>
          <BinaryState>0</BinaryState>
        </e:property>
      </e:propertyset>"#;

    assert_eq!(WemoState::Off, parse_state(xml).unwrap());

    let xml = r#"
      <e:propertyset xmlns:e="\#urn:schemas-upnp-org:event-1-0">
        <e:property>
          <BinaryState>1</BinaryState>
        </e:property>
      </e:propertyset>"#;

    assert_eq!(WemoState::On, parse_state(xml).unwrap());
  }

  #[test]
  fn insight_notifications() {
    let xml = r#"
      <e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
        <e:property>
          <BinaryState>0|1234567890|1234|4321|111111|1234567|11|55555|6543210|000000000</BinaryState>
        </e:property>
      </e:propertyset>"#;

    assert_eq!(WemoState::Off, parse_state(xml).unwrap());

    let xml = r#"
      <e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
        <e:property>
          <BinaryState>1|1234567890|1234|4321|111111|1234567|11|55555|6543210|000000000</BinaryState>
        </e:property>
      </e:propertyset>"#;

    assert_eq!(WemoState::On, parse_state(xml).unwrap());

    let xml = r#"
      <e:propertyset xmlns:e="urn:schemas-upnp-org:event-1-0">
        <e:property>
          <BinaryState>8|1234567890|1234|4321|111111|1234567|11|55555|6543210|000000000</BinaryState>
        </e:property>
      </e:propertyset>"#;

    assert_eq!(WemoState::OnWithoutLoad, parse_state(xml).unwrap());
  }
}
