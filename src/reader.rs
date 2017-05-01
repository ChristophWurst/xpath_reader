//! Main XPath reader code.

use sxd_document::Package;
use sxd_document::parser::parse as sxd_parse;
use sxd_xpath::{Value, Factory, XPath};
use sxd_xpath::nodeset::Node;
use sxd_xpath::Value::Nodeset;

use super::{XpathError, XpathErrorKind};
use super::context::Context;

/// A trait to abstract the idea of something that can be parsed from XML.
pub trait FromXml
    where Self: Sized
{
    /// Read an instance of `Self` from the provided `reader`.
    ///
    /// The reader can be relative to a specific element. Whether the root of the document contains
    /// the element to be parsed or is the element to be parsed can be specified by the additional
    /// traits `FromXmlContained` and `FromXmlElement`.
    fn from_xml<'d, R>(reader: &'d R) -> Result<Self, XpathError> where R: XpathReader<'d>;
}

/// Similar to `FromXml` abstracts the idea of an `Option` of something that can be parsed from XML.
pub trait OptionFromXml
    where Self: Sized
{
    fn option_from_xml<'d, R>(reader: &'d R) -> Result<Option<Self>, XpathError>
        where R: XpathReader<'d>;
}

/// `FromXml` takes a reader as input whose root element **contains** the relevant element.
pub trait FromXmlContained: FromXml {}

/// `FromXml` takes a reader as input whose root element **is** the relevant element.
pub trait FromXmlElement: FromXml {}

/// Allows to execute XPath expressions on some kind of document.
///
/// Different implementors have different root nodes.
pub trait XpathReader<'d> {
    /// Evaluate an Xpath expression on the root of this reader.
    ///
    /// Normally you won't have to use this method at all and use `read`, `read_option` or
    /// `read_vec` instead.
    fn evaluate(&'d self, xpath_expr: &str) -> Result<Value<'d>, XpathError>;

    /// Returns a reference to the `Context` used by the reader instance.
    fn context(&'d self) -> &'d Context<'d>;

    /// Read the result of the xpath expression into a value of type `V`.
    fn read<V>(&'d self, xpath_expr: &str) -> Result<V, XpathError>
        where V: FromXml
    {
        let reader = self.relative(xpath_expr)?;
        V::from_xml(&reader)
    }

    /// Read the result of the xpath expression into a value of type `Option<V>`.
    fn read_option<V>(&'d self, xpath_expr: &str) -> Result<Option<V>, XpathError>
        where V: OptionFromXml
    {
        match self.relative(xpath_expr) {
            Ok(reader) => V::option_from_xml(&reader),
            Err(XpathError(XpathErrorKind::NodeNotFound(_), _)) => Ok(None),
            Err(e) => Err(e)
        }
    }

    /// Execute an Xpath expression and parse the result into a vector of `Item` instances.
    fn read_vec<Item>(&'d self, xpath_expr: &str) -> Result<Vec<Item>, XpathError>
        where Item: FromXml
    {
        match self.evaluate(xpath_expr)? {
            Nodeset(nodeset) => {
                nodeset.document_order()
                    .iter()
                    .map(|node| {
                        XpathNodeReader::new(*node, self.context()).and_then(|r| Item::from_xml(&r))
                    })
                    .collect()
            }
            _ => Ok(Vec::new()),
        }
    }

    /// Evaluates an Xpath query, takes the first returned node (in document order) and creates
    /// a new XpathNodeReader with that node.
    fn relative(&'d self, xpath_expr: &str) -> Result<XpathNodeReader<'d>, XpathError> {
        let node: Node<'d> = match self.evaluate(xpath_expr)? {
            Value::Nodeset(nodeset) => {
                let res: Result<Node<'d>, XpathError> = nodeset.document_order_first()
                    .ok_or_else(|| {
                        XpathErrorKind::NodeNotFound(xpath_expr.to_string()).into()
                    });
                res?
            }
            _ => return Err(format!("XPath didn't specify a nodeset: '{}'", xpath_expr).into()),
        };
        XpathNodeReader::new(node, self.context())
    }
}

/// Reader that parses an XML string and runs expressions against its root element.
pub struct XpathStrReader<'d> {
    context: &'d Context<'d>,
    factory: Factory,
    package: Package,
}

impl<'d> XpathStrReader<'d> {
    pub fn new(xml: &str, context: &'d Context<'d>) -> Result<Self, XpathError> {
        Ok(Self {
            context: context,
            factory: Factory::default(),
            package: sxd_parse(xml)?,
        })
    }
}

fn build_xpath(factory: &Factory, xpath_expr: &str) -> Result<XPath, XpathError> {
    factory.build(xpath_expr)?
        .ok_or_else(|| "Xpath instance was `None`!".into())
}

impl<'d> XpathReader<'d> for XpathStrReader<'d> {
    fn evaluate(&'d self, xpath_expr: &str) -> Result<Value<'d>, XpathError> {
        let xpath = build_xpath(&self.factory, xpath_expr)?;
        xpath.evaluate(&self.context, self.package.as_document().root()).map_err(XpathError::from)
    }

    fn context(&'d self) -> &'d Context<'d> {
        &self.context
    }
}

/// Reader that takes another node as input and allows parsing against this node as root.
pub struct XpathNodeReader<'d> {
    factory: Factory,
    node: Node<'d>,
    context: &'d Context<'d>,
}

impl<'d> XpathNodeReader<'d> {
    pub fn new<N>(node: N, context: &'d Context<'d>) -> Result<Self, XpathError>
        where N: Into<Node<'d>>
    {
        Ok(Self {
            node: node.into(),
            factory: Factory::default(),
            context: context,
        })
    }
}

impl<'d> XpathReader<'d> for XpathNodeReader<'d> {
    fn evaluate(&'d self, xpath_expr: &str) -> Result<Value<'d>, XpathError> {
        let xpath = build_xpath(&self.factory, xpath_expr)?;
        xpath.evaluate(self.context, self.node).map_err(XpathError::from)
    }

    fn context(&'d self) -> &'d Context<'d> {
        self.context
    }
}

impl FromXmlElement for String {}

impl FromXml for String {
    fn from_xml<'d, R>(reader: &'d R) -> Result<Self, XpathError>
        where R: XpathReader<'d>
    {
        Ok(reader.evaluate(".")?.string())
    }
}

impl OptionFromXml for String {
    /// An empty string is parsed to `None` while any other string is parsed to `Some(String)`
    /// containig the string value.
    fn option_from_xml<'d, R>(reader: &'d R) -> Result<Option<Self>, XpathError>
        where R: XpathReader<'d>
    {
        let s = String::from_xml(reader)?;
        if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
    }
}

macro_rules! from_float_types {
    ( $( $type:ty ),* ) => {
        $(
            impl FromXmlElement for $type { }

            impl FromXml for $type {
                fn from_xml<'d, R>(reader: &'d R) -> Result<Self, XpathError>
                    where R: XpathReader<'d>
                {
                    let num = reader.evaluate(".")?.number();
                    Ok(num as $type)
                }
            }
        )*
    }
}

from_float_types!(f32, f64);

macro_rules! from_parse_str {
    ( $( $type:ty ),* ) => {
        $(
            impl FromXmlElement for $type { }

            impl FromXml for $type {
                fn from_xml<'d, R>(reader: &'d R) -> Result<Self, XpathError>
                    where R: XpathReader<'d>
                {
                    let s = String::from_xml(reader)?;
                    Ok(s.parse()?)
                }
            }
        )*
    }
}

from_parse_str!(u8, u16, u32, u64, i8, i16, i32, i64, bool);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xpath_str_reader() {
        let context = Context::new();
        let xml =
            r#"<?xml version="1.0" encoding="UTF-8"?><root><child name="Hello World"/></root>"#;
        let reader = XpathStrReader::new(xml, &context).unwrap();
        assert_eq!(reader.evaluate(".//child/@name").unwrap().string(),
                   "Hello World".to_string());
    }

    const XML_STRING: &str =
        r#"<?xml version="1.0"?><root><title>Hello World</title><empty/></root>"#;

    #[test]
    fn string_option_from_xml() {
        let context = Context::new();
        let reader = XpathStrReader::new(XML_STRING, &context).unwrap();

        let title = reader.relative("//title").unwrap();
        assert_eq!(String::option_from_xml(&title).unwrap(),
                   Some("Hello World".to_string()));

        let empty = reader.relative("//empty").unwrap();
        assert_eq!(String::option_from_xml(&empty).unwrap(), None);

    }

    #[test]
    fn string_from_xml() {
        let context = Context::new();
        let reader = XpathStrReader::new(XML_STRING, &context).unwrap();

        let title = reader.relative("//title").unwrap();
        assert_eq!(String::from_xml(&title).unwrap(), "Hello World".to_string());

        let empty = reader.relative("//empty").unwrap();
        assert_eq!(String::from_xml(&empty).unwrap(), "".to_string());
    }

    #[test]
    fn num_from_xml() {
        let xml = r#"<?xml version="1.0"?><root><float>-23.85</float><int>42</int></root>"#;
        let context = Context::new();
        let reader = XpathStrReader::new(xml, &context).unwrap();

        let float = reader.relative("//float").unwrap();
        let int = reader.relative("//int").unwrap();

        assert_eq!(f32::from_xml(&float).unwrap(), -23.85f32);
        assert_eq!(f32::from_xml(&int).unwrap(), 42f32);
        assert_eq!(f64::from_xml(&float).unwrap(), -23.85f64);
        assert_eq!(f64::from_xml(&int).unwrap(), 42f64);

        assert_eq!(u8::from_xml(&int).unwrap(), 42u8);
        assert_eq!(u16::from_xml(&int).unwrap(), 42u16);
        assert_eq!(u32::from_xml(&int).unwrap(), 42u32);
        assert_eq!(u64::from_xml(&int).unwrap(), 42u64);

        assert_eq!(i8::from_xml(&int).unwrap(), 42i8);
        assert_eq!(i16::from_xml(&int).unwrap(), 42i16);
        assert_eq!(i32::from_xml(&int).unwrap(), 42i32);
        assert_eq!(i64::from_xml(&int).unwrap(), 42i64);
    }

    #[test]
    fn bool_from_xml() {
        let xml = r#"<?xml version="1.0"?><root><t>true</t><f>false</f></root>"#;
        let context = Context::new();
        let reader = XpathStrReader::new(xml, &context).unwrap();

        let t = reader.relative("//t").unwrap();
        let f = reader.relative("//f").unwrap();

        assert_eq!(bool::from_xml(&t).unwrap(), true);
        assert_eq!(bool::from_xml(&f).unwrap(), false);
    }
}