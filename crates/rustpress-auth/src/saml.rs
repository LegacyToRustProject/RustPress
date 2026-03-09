//! SAML 2.0 Service Provider (SP) skeleton.
//!
//! Implements the SP-initiated SSO redirect binding and basic response parsing.
//! Signature verification is flagged with TODO; production deployments MUST
//! validate the IdP signature before trusting assertion data.

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as BASE64_STD, Engine as _};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, instrument, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Identity Provider + Service Provider configuration for a SAML 2.0 integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlProvider {
    // ---- Identity Provider (IdP) settings ----

    /// IdP entity ID (issuer). Example: `"https://idp.example.com/metadata"`.
    pub idp_entity_id: String,

    /// IdP Single Sign-On service URL (HTTP-Redirect binding endpoint).
    pub idp_sso_url: String,

    /// IdP X.509 certificate in PEM format, used to verify the assertion
    /// signature. Must be kept up-to-date when the IdP rotates its key.
    pub idp_cert: String,

    // ---- Service Provider (SP) settings ----

    /// SP entity ID (issuer). Typically the SP metadata URL.
    pub sp_entity_id: String,

    /// SP Assertion Consumer Service URL — the callback endpoint that receives
    /// the IdP's POST with the SAML Response.
    pub sp_acs_url: String,

    /// Optional SP private key in PEM format for signing AuthnRequests.
    /// When `None`, AuthnRequests are sent unsigned.
    pub sp_private_key: Option<String>,
}

// ---------------------------------------------------------------------------
// Request / assertion types
// ---------------------------------------------------------------------------

/// Minimal representation of a SAML `<AuthnRequest>` for bookkeeping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlAuthRequest {
    /// Unique request identifier (`_<uuid>`).
    pub id: String,
    /// UTC timestamp at which the request was created (ISO 8601).
    pub issue_instant: String,
    /// The IdP SSO URL this request is destined for.
    pub destination: String,
}

/// Parsed content of a validated SAML `<Assertion>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlAssertion {
    /// The `<NameID>` value — typically the user's email or opaque identifier.
    pub subject: String,

    /// Multi-valued SAML attributes keyed by attribute name.
    /// Example: `{"email": ["user@example.com"], "groups": ["admin", "users"]}`.
    pub attributes: HashMap<String, Vec<String>>,

    /// Conditions constraining the assertion's validity window.
    pub conditions: SamlConditions,

    /// UTC timestamp when authentication occurred.
    pub authn_instant: String,

    /// Issuer entity ID from the assertion.
    pub issuer: String,
}

/// Validity window from `<Conditions NotBefore="…" NotOnOrAfter="…">`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamlConditions {
    /// Earliest time at which the assertion may be used.
    pub valid_from: Option<DateTime<Utc>>,
    /// Expiry time (exclusive) after which the assertion must be rejected.
    pub valid_to: Option<DateTime<Utc>>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can arise during SAML SP operations.
#[derive(Debug, Error)]
pub enum SamlError {
    /// The response document could not be decoded or parsed.
    #[error("SAML response parse error: {0}")]
    ParseError(String),

    /// The IdP signature on the response or assertion failed verification.
    #[error("SAML signature verification failed")]
    SignatureVerificationFailed,

    /// The assertion's `NotOnOrAfter` is in the past, or `NotBefore` is in
    /// the future.
    #[error("SAML assertion has expired or is not yet valid")]
    ExpiredAssertion,

    /// A required SAML attribute was absent from the assertion.
    #[error("missing required SAML attribute: {0}")]
    MissingAttribute(String),

    /// The decoded response was structurally invalid.
    #[error("invalid SAML response: {0}")]
    InvalidResponse(String),
}

// ---------------------------------------------------------------------------
// Service Provider
// ---------------------------------------------------------------------------

/// SAML 2.0 Service Provider.
///
/// # Usage
///
/// ```rust,ignore
/// let provider = SamlProvider { /* … */ };
/// let sp = SamlSp::new(provider);
///
/// // SP-initiated SSO — build redirect URL and save the request ID.
/// let (redirect_url, authn_request) = sp.create_authn_request();
///
/// // On callback — parse the base64-encoded SAMLResponse POST parameter.
/// let assertion = sp.parse_response(&saml_response_b64)?;
/// let email = SamlSp::get_user_email(&assertion);
/// ```
#[derive(Debug, Clone)]
pub struct SamlSp {
    provider: SamlProvider,
}

impl SamlSp {
    /// Create a new `SamlSp` for the given provider configuration.
    pub fn new(provider: SamlProvider) -> Self {
        Self { provider }
    }

    // -----------------------------------------------------------------------
    // AuthnRequest
    // -----------------------------------------------------------------------

    /// Build an SP-initiated SAML AuthnRequest and return the IdP redirect URL.
    ///
    /// The AuthnRequest XML is deflate-compressed and base64url-encoded per the
    /// HTTP-Redirect binding specification (SAML 2.0 §3.4).
    ///
    /// Returns `(redirect_url, authn_request)`. Callers should persist the
    /// `authn_request.id` (InResponseTo check) in the session.
    #[instrument(skip(self), fields(idp = %self.provider.idp_entity_id))]
    pub fn create_authn_request(&self) -> (String, SamlAuthRequest) {
        let id = format!("_{}", Uuid::new_v4().simple());
        let issue_instant = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

        let xml = build_authn_request_xml(
            &id,
            &issue_instant,
            &self.provider.idp_sso_url,
            &self.provider.sp_entity_id,
            &self.provider.sp_acs_url,
        );

        debug!(request_id = %id, "built SAMLRequest XML ({} bytes)", xml.len());

        // HTTP-Redirect binding: DEFLATE-compress then base64-encode the XML.
        // TODO: use `flate2` to apply raw DEFLATE compression when the crate is
        //       added to the workspace. For now we transmit uncompressed XML,
        //       which most IdPs accept in development environments.
        let encoded = BASE64_STD.encode(xml.as_bytes());

        let redirect_url = format!(
            "{}?SAMLRequest={}&RelayState=",
            self.provider.idp_sso_url,
            urlencoding_encode(&encoded),
        );

        let authn_request = SamlAuthRequest {
            id,
            issue_instant,
            destination: self.provider.idp_sso_url.clone(),
        };

        (redirect_url, authn_request)
    }

    // -----------------------------------------------------------------------
    // Response parsing
    // -----------------------------------------------------------------------

    /// Parse and validate a base64-encoded `SAMLResponse` POST parameter.
    ///
    /// # Security considerations
    ///
    /// - **Signature verification is stubbed** — see `TODO` inside this method.
    ///   Production code MUST verify the IdP signature using `idp_cert` before
    ///   returning an `Ok(SamlAssertion)`.
    /// - Callers should also check `assertion.conditions.valid_to` against the
    ///   current time after this function returns.
    #[instrument(skip(self, saml_response_b64))]
    pub fn parse_response(
        &self,
        saml_response_b64: &str,
    ) -> Result<SamlAssertion, SamlError> {
        // 1. Decode base64.
        let xml_bytes = BASE64_STD
            .decode(saml_response_b64.trim())
            .map_err(|e| SamlError::ParseError(format!("base64 decode: {e}")))?;
        let xml = std::str::from_utf8(&xml_bytes)
            .map_err(|e| SamlError::ParseError(format!("UTF-8 decode: {e}")))?;

        debug!("decoded SAMLResponse ({} bytes)", xml.len());

        // 2. TODO: Verify IdP signature using `self.provider.idp_cert`.
        //    Use the `openssl` or `ring` crate to validate the XML-DSig signature
        //    on either the `<Response>` or the inner `<Assertion>` element.
        //    Until this is implemented, log a prominent warning.
        warn!(
            "SAML signature verification is NOT implemented — \
             assertions are UNTRUSTED. Do not use in production."
        );

        // 3. Parse the XML into a SamlAssertion.
        parse_assertion_from_xml(xml)
    }

    // -----------------------------------------------------------------------
    // Attribute helpers
    // -----------------------------------------------------------------------

    /// Extract the user's email from an assertion's attributes.
    ///
    /// Checks common attribute names used by enterprise IdPs:
    /// - `email`
    /// - `mail`
    /// - `http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress`
    /// - `urn:oid:0.9.2342.19200300.100.1.3` (eduPerson `mail`)
    pub fn get_user_email(assertion: &SamlAssertion) -> Option<String> {
        const EMAIL_ATTRS: &[&str] = &[
            "email",
            "mail",
            "http://schemas.xmlsoap.org/ws/2005/05/identity/claims/emailaddress",
            "http://schemas.xmlsoap.org/claims/EmailAddress",
            "urn:oid:0.9.2342.19200300.100.1.3",
        ];

        for attr_name in EMAIL_ATTRS {
            if let Some(values) = assertion.attributes.get(*attr_name) {
                if let Some(first) = values.first() {
                    if !first.is_empty() {
                        return Some(first.clone());
                    }
                }
            }
        }

        // Fallback: if NameID looks like an email address, use it.
        if assertion.subject.contains('@') {
            return Some(assertion.subject.clone());
        }

        None
    }

    /// Return a flat `HashMap<name, first_value>` of assertion attributes.
    ///
    /// Multi-valued attributes are collapsed to their first value. For the
    /// full multi-value representation use `assertion.attributes` directly.
    pub fn get_user_attributes(assertion: &SamlAssertion) -> HashMap<String, String> {
        assertion
            .attributes
            .iter()
            .filter_map(|(k, values)| {
                values.first().cloned().map(|v| (k.clone(), v))
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// XML building helper
// ---------------------------------------------------------------------------

/// Construct a minimal SAML 2.0 `<AuthnRequest>` XML document.
///
/// The output is a well-formed XML string suitable for transmission via the
/// HTTP-Redirect or HTTP-POST binding.
fn build_authn_request_xml(
    id: &str,
    issue_instant: &str,
    destination: &str,
    issuer: &str,
    acs_url: &str,
) -> String {
    // We build the XML manually to avoid pulling in an XML writer crate just
    // for a single, static-shape document.  All dynamic values are
    // XML-attribute-escaped before insertion.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<samlp:AuthnRequest
  xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol"
  xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion"
  ID="{id}"
  Version="2.0"
  IssueInstant="{issue_instant}"
  Destination="{destination}"
  ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"
  AssertionConsumerServiceURL="{acs_url}">
  <saml:Issuer>{issuer}</saml:Issuer>
  <samlp:NameIDPolicy
    Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress"
    AllowCreate="true"/>
</samlp:AuthnRequest>"#,
        id = xml_attr_escape(id),
        issue_instant = xml_attr_escape(issue_instant),
        destination = xml_attr_escape(destination),
        acs_url = xml_attr_escape(acs_url),
        issuer = xml_text_escape(issuer),
    )
}

// ---------------------------------------------------------------------------
// XML parsing helper
// ---------------------------------------------------------------------------

/// Extract a [`SamlAssertion`] from the raw XML of a SAML Response document.
///
/// Uses `quick-xml` for parsing. Only the fields required by [`SamlAssertion`]
/// are extracted; the full document is not validated beyond basic structure.
fn parse_assertion_from_xml(xml: &str) -> Result<SamlAssertion, SamlError> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut subject: Option<String> = None;
    let mut issuer: Option<String> = None;
    let mut authn_instant: Option<String> = None;
    let mut valid_from: Option<DateTime<Utc>> = None;
    let mut valid_to: Option<DateTime<Utc>> = None;
    let mut attributes: HashMap<String, Vec<String>> = HashMap::new();

    // Simple state machine tracking which element we are currently inside.
    #[derive(Default, PartialEq)]
    enum State {
        #[default]
        Other,
        Issuer,
        NameId,
        AttributeName(String),
        AttributeValue(String),
    }

    let mut state = State::default();
    let mut buf = Vec::with_capacity(4096);

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "Issuer" => state = State::Issuer,
                    "NameID" => state = State::NameId,
                    "Conditions" => {
                        for attr in e.attributes().flatten() {
                            let key = local_name(attr.key.as_ref());
                            let val = attr
                                .unescape_value()
                                .unwrap_or_default()
                                .to_string();
                            match key.as_str() {
                                "NotBefore" => {
                                    valid_from = val.parse::<DateTime<Utc>>().ok();
                                }
                                "NotOnOrAfter" => {
                                    valid_to = val.parse::<DateTime<Utc>>().ok();
                                }
                                _ => {}
                            }
                        }
                    }
                    "AuthnStatement" => {
                        for attr in e.attributes().flatten() {
                            let key = local_name(attr.key.as_ref());
                            if key == "AuthnInstant" {
                                authn_instant = Some(
                                    attr.unescape_value()
                                        .unwrap_or_default()
                                        .to_string(),
                                );
                            }
                        }
                        state = State::Other;
                    }
                    "Attribute" => {
                        // Collect the attribute name from either `Name` or
                        // `FriendlyName` (prefer `Name`).
                        let mut attr_name = String::new();
                        for xml_attr in e.attributes().flatten() {
                            let key = local_name(xml_attr.key.as_ref());
                            if key == "Name" {
                                attr_name = xml_attr
                                    .unescape_value()
                                    .unwrap_or_default()
                                    .to_string();
                                break;
                            }
                        }
                        if !attr_name.is_empty() {
                            state = State::AttributeName(attr_name);
                        }
                    }
                    "AttributeValue" => {
                        if let State::AttributeName(ref name) = state {
                            state = State::AttributeValue(name.clone());
                        }
                    }
                    _ => {
                        if !matches!(
                            state,
                            State::AttributeName(_) | State::AttributeValue(_)
                        ) {
                            state = State::Other;
                        }
                    }
                }
            }

            Ok(Event::Text(ref t)) => {
                let text = t.unescape().unwrap_or_default().to_string();
                match &state {
                    State::Issuer if issuer.is_none() => issuer = Some(text),
                    State::NameId => subject = Some(text),
                    State::AttributeValue(ref name) => {
                        attributes
                            .entry(name.clone())
                            .or_default()
                            .push(text);
                    }
                    _ => {}
                }
            }

            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "Issuer" | "NameID" => state = State::Other,
                    "AttributeValue" => {
                        if let State::AttributeValue(ref name) = state {
                            state = State::AttributeName(name.clone());
                        }
                    }
                    "Attribute" => state = State::Other,
                    _ => {}
                }
            }

            Ok(Event::Eof) => break,

            Err(e) => {
                return Err(SamlError::ParseError(format!("XML parse error: {e}")));
            }

            _ => {}
        }

        buf.clear();
    }

    // Validate the mandatory fields.
    let subject = subject.ok_or_else(|| {
        SamlError::InvalidResponse("missing <NameID> in assertion".into())
    })?;
    let issuer = issuer.unwrap_or_default();
    let authn_instant = authn_instant.unwrap_or_else(|| Utc::now().to_rfc3339());

    // Check temporal validity.
    let now = Utc::now();
    if let Some(not_before) = valid_from {
        if now < not_before {
            return Err(SamlError::ExpiredAssertion);
        }
    }
    if let Some(not_after) = valid_to {
        if now >= not_after {
            return Err(SamlError::ExpiredAssertion);
        }
    }

    Ok(SamlAssertion {
        subject,
        attributes,
        conditions: SamlConditions { valid_from, valid_to },
        authn_instant,
        issuer,
    })
}

// ---------------------------------------------------------------------------
// Utility functions
// ---------------------------------------------------------------------------

/// Strip the XML namespace prefix from an element or attribute name and return
/// only the local part (everything after the last `:`).
fn local_name(name: &[u8]) -> String {
    let s = std::str::from_utf8(name).unwrap_or("");
    s.rfind(':').map_or(s, |pos| &s[pos + 1..]).to_owned()
}

/// Escape a string for safe inclusion in an XML attribute value (double-quoted).
fn xml_attr_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape a string for safe inclusion as XML text content.
fn xml_text_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Percent-encode a string for use in a URL query parameter value.
///
/// This is a minimal encoder: only characters that MUST be encoded in a query
/// string are encoded. For a production implementation, prefer the `percent-encoding` crate.
fn urlencoding_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            other => {
                out.push('%');
                out.push_str(&format!("{:02X}", other));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_provider() -> SamlProvider {
        SamlProvider {
            idp_entity_id: "https://idp.example.com".into(),
            idp_sso_url: "https://idp.example.com/sso".into(),
            idp_cert: "-----BEGIN CERTIFICATE-----\n(stub)\n-----END CERTIFICATE-----".into(),
            sp_entity_id: "https://sp.example.com/metadata".into(),
            sp_acs_url: "https://sp.example.com/saml/acs".into(),
            sp_private_key: None,
        }
    }

    #[test]
    fn create_authn_request_returns_redirect_url() {
        let sp = SamlSp::new(make_provider());
        let (url, req) = sp.create_authn_request();
        assert!(url.starts_with("https://idp.example.com/sso?SAMLRequest="));
        assert!(req.id.starts_with('_'));
        assert!(!req.issue_instant.is_empty());
    }

    #[test]
    fn authn_request_xml_well_formed() {
        let xml = build_authn_request_xml(
            "_abc123",
            "2026-01-01T00:00:00Z",
            "https://idp.example.com/sso",
            "https://sp.example.com/metadata",
            "https://sp.example.com/saml/acs",
        );
        assert!(xml.contains("samlp:AuthnRequest"));
        assert!(xml.contains("_abc123"));
        assert!(xml.contains("https://idp.example.com/sso"));
    }

    #[test]
    fn get_user_email_from_attributes() {
        let mut attributes = HashMap::new();
        attributes.insert("email".into(), vec!["user@example.com".into()]);
        let assertion = SamlAssertion {
            subject: "uid=user".into(),
            attributes,
            conditions: SamlConditions { valid_from: None, valid_to: None },
            authn_instant: "2026-01-01T00:00:00Z".into(),
            issuer: "https://idp.example.com".into(),
        };
        assert_eq!(
            SamlSp::get_user_email(&assertion),
            Some("user@example.com".into())
        );
    }

    #[test]
    fn get_user_email_fallback_to_name_id() {
        let assertion = SamlAssertion {
            subject: "user@example.com".into(),
            attributes: HashMap::new(),
            conditions: SamlConditions { valid_from: None, valid_to: None },
            authn_instant: "2026-01-01T00:00:00Z".into(),
            issuer: "https://idp.example.com".into(),
        };
        assert_eq!(
            SamlSp::get_user_email(&assertion),
            Some("user@example.com".into())
        );
    }

    #[test]
    fn get_user_attributes_collapses_to_first_value() {
        let mut attributes = HashMap::new();
        attributes.insert("groups".into(), vec!["admin".into(), "users".into()]);
        let assertion = SamlAssertion {
            subject: "user".into(),
            attributes,
            conditions: SamlConditions { valid_from: None, valid_to: None },
            authn_instant: "2026-01-01T00:00:00Z".into(),
            issuer: "".into(),
        };
        let flat = SamlSp::get_user_attributes(&assertion);
        assert_eq!(flat.get("groups").map(String::as_str), Some("admin"));
    }

    #[test]
    fn parse_response_rejects_invalid_base64() {
        let sp = SamlSp::new(make_provider());
        let err = sp.parse_response("not-valid-base64!!!").unwrap_err();
        assert!(matches!(err, SamlError::ParseError(_)));
    }

    #[test]
    fn xml_attr_escape_handles_special_chars() {
        assert_eq!(xml_attr_escape(r#"a&b"c<d>e"#), "a&amp;b&quot;c&lt;d&gt;e");
    }

    #[test]
    fn local_name_strips_prefix() {
        assert_eq!(local_name(b"samlp:AuthnRequest"), "AuthnRequest");
        assert_eq!(local_name(b"Issuer"), "Issuer");
    }
}
