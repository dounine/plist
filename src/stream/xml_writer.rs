use crate::plist::Plist;

pub trait XmlWriter {
    fn convert_xml(&self, indent: usize) -> String;
}
impl XmlWriter for Plist {
    fn convert_xml(&self, indent: usize) -> String {
        let indent_str = "\t".repeat(indent);
        let mut xml = String::new();
        match self {
            Plist::Float(value) => xml.push_str(&format!("{}<real>{}</real>\n", indent_str, value)),
            Plist::Array(list) => {
                xml.push_str(&format!("{}<array>\n", indent_str));
                for item in list {
                    xml.push_str(&item.convert_xml(indent + 1));
                }
                xml.push_str(&format!("{}</array>\n", indent_str));
            }
            Plist::Dictionary(dict) => {
                xml.push_str(&format!("{}<dict>\n", indent_str));
                for (key, value) in dict {
                    xml.push_str(&format!("\t{}<key>{}</key>\n", indent_str, key));
                    xml.push_str(&value.convert_xml(indent + 1)); // 递归增加缩进
                }
                xml.push_str(&format!("{}</dict>\n", indent_str));
            }
            Plist::Boolean(value) => {
                if *value {
                    xml.push_str(&format!("{}<true/>\n", indent_str))
                } else {
                    xml.push_str(&format!("{}<false/>\n", indent_str))
                }
            }
            Plist::Integer(value) => {
                xml.push_str(&format!("{}<integer>{}</integer>\n", indent_str, value))
            }
            Plist::String(value) => {
                xml.push_str(&format!("{}<string>{}</string>\n", indent_str, value))
            }
            Plist::Date(value) => xml.push_str(&format!("{}<date>{}</date>\n", indent_str, value)),
            Plist::Data(value) => {
                let value = String::from_utf8_lossy(value).to_string();
                xml.push_str(&format!("{}<data>{}</data>\n", indent_str, value))
            }
        }
        xml
    }
}
