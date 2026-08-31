//! HTML Entity Encoding & Decoding using standard official `html_escape` crate

pub fn encode(input: &str) -> String {
    html_escape::encode_text(input).into_owned()
}

pub fn decode(input: &str) -> Result<String, String> {
    Ok(html_escape::decode_html_entities(input).into_owned())
}
