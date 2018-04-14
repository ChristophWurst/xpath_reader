//! Errors used in this crate.
//! We are using `error-chain` so if you are using it too you can just add a link for this crate's
//! errors.

use std::error::Error;
use std::fmt;

error_chain! {
    types {
        XpathError, XpathErrorKind, ChainXpathErr, XpathResult;
    }

    foreign_links {
        XmlParseError(::sxd_document::parser::Error);
        XpathError(::sxd_xpath::Error);
        XpathExecuteError(::sxd_xpath::ExecutionError);
        XpathParseError(::sxd_xpath::ParserError);
    }

    errors {
        /// XPath expression failed to evaluate to a value.
        /// The String variant contains a copy of the XPath expression.
        NodeNotFound(xpath: String) {
            description("XPath expression didn't yield a node.")
            display("XPath expression '{}' failed to find a node.", xpath)
        }

        /// Conversion from XML failed,
        /// used for custom failures in `FromXml` and `OptionFromXml` traits.
        FromXmlError(err: Box<Error + Send>) {
            description("Conversion from XML failed.")
            display("Conversion from XML failed: {:?}", err)
        }

        MissingValue(info: String) {
            description("A required value was missing in the document.")
            display("A required value was missing from the document: {}", info)
        }
    }
}

/// An Error which can occur during the conversion of types from XML.
#[derive(Debug)]
pub enum FromXmlError {
    // The value was not found in the document.
    //Absent,

    /// Any error other than absence of a value occuring during conversion of a type from XML.
    Other(XpathError),
}

impl<E> From<E> for FromXmlError
where
    E: Into<XpathError>,
{
    fn from(e: E) -> Self {
        FromXmlError::Other(e.into())
    }
}

impl FromXmlError {
    pub fn into_xpath_error(self) -> XpathError {
        match self {
            FromXmlError::Other(err) => err,
        }
    }
}

impl Error for FromXmlError {
    fn description(&self) -> &str {
        "There was an error converting this type from XML."
    }
}

impl fmt::Display for FromXmlError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FromXmlError::Other(ref e) => e.fmt(f),
        }
    }
}

macro_rules! from_xml_error {
    ( $( $type:ty );* ; ) => {
        $(
            impl From<$type> for XpathError {
                fn from(err: $type) -> XpathError {
                    XpathErrorKind::FromXmlError(Box::new(err)).into()
                }
            }
        )*
    }
}

from_xml_error!(
    ::std::str::ParseBoolError;
    ::std::num::ParseIntError;
    ::std::num::ParseFloatError;
);

// TODO: Take this upstream, either the tuple should implement std::Error or another type should be
// used which does.
impl From<(usize, ::std::vec::Vec<::sxd_document::parser::Error>)> for XpathError {
    fn from(err: (usize, ::std::vec::Vec<::sxd_document::parser::Error>)) -> XpathError {
        XpathErrorKind::XmlParseError(err.1[0]).into()
    }
}
