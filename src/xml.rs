// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

use regex::Regex;

/// Super lazy way to extract text between tags without real XML parsing.
/// (Better hope for no duplicate tags, nested tags, or anything really...!)
pub fn find_tag_value<'a>(tag_name: &str, xml: &'a str) -> Option<&'a str> {
  let reg = format!(r"(?im:<{}>(.*)</{}>)", tag_name, tag_name);
  let re = Regex::new(reg.as_ref()).unwrap();

  for capture in re.captures_iter(xml) {
    return capture.at(1);
  }
  None
}

#[cfg(test)]
mod tests {
  use super::*;

  const XML: &'static str = " \
    <?xml version=\"1.0\" encoding=\"utf-8\"?> \
      <s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
          s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"> \
        <s:Body> \
          <u:GetBinaryState xmlns:u=\"urn:Belkin:service:basicevent:1\"> \
            <BinaryState>1</BinaryState> \
          </u:GetBinaryState> \
        </s:Body> \
      </s:Envelope> \
    ";

  #[test]
  fn test_find_tag_value() {
    assert_eq!("1", find_tag_value("BinaryState", XML).unwrap());

    assert_eq!("Pikachu",
      find_tag_value("pokemon", "<pokemon>Pikachu</pokemon>").unwrap());
  }

  #[test]
  fn test_find_tag_value_failure() {
    assert_eq!(None, find_tag_value("TernaryState", XML));

    assert_eq!(None,
      find_tag_value("futuramaCharacter", "<pokemon>Pikachu</pokemon>"));
  }
}
