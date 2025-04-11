use crate::bplist00::BPlist00;
use crate::error::Error;
use crate::write::bplist_write;
use crate::write::bplist_write::BPlistWrite;
use crate::xml::XmlPlist;
use chrono::{DateTime, Utc};
use std::collections::BTreeMap;
use std::io::Cursor;

#[derive(Debug, Clone)]
pub enum Plist {
    Array(Vec<Plist>),
    Dictionary(Vec<(String, Plist)>),
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Date(DateTime<Utc>),
    Data(Vec<u8>),
}
impl Plist {
    pub fn parse(data: &[u8]) -> Result<Self, Error> {
        if data.starts_with(b"bplist00") {
            let (_, value) = BPlist00::parse(data).map_err(|e| Error::Error(e.to_string()))?;
            Ok(value)
        } else {
            XmlPlist::parse(data)
        }
    }
}
impl From<bool> for Plist {
    fn from(value: bool) -> Self {
        Plist::Boolean(value)
    }
}
impl From<i64> for Plist {
    fn from(value: i64) -> Self {
        Plist::Integer(value)
    }
}
impl From<&str> for Plist {
    fn from(value: &str) -> Self {
        Plist::String(value.to_string())
    }
}
impl From<String> for Plist {
    fn from(value: String) -> Self {
        Plist::String(value)
    }
}
#[allow(dead_code)]
impl Plist {
    pub fn to_binary(&self) -> Result<Vec<u8>, Error> {
        let mut plist_write = BPlistWrite::new();
        let mut output = Cursor::new(vec![]);
        plist_write.write(self, &mut output)?;
        Ok(output.into_inner())
    }
    pub fn to_xml(&self) -> String {
        let mut xml = String::from(
            r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
"#,
        );

        xml.push_str(&self.convert_xml(0));
        xml.push_str("</plist>");
        xml
    }
    pub fn sort_key(&mut self) {
        if let Plist::Dictionary(dict) = self {
            dict.sort_by(|(a_key, _), (b_key, _)| a_key.cmp(b_key));
            // let mut sorted_keys: Vec<String> = dict.iter().map(|(k, v)| k).cloned().collect();
            // sorted_keys.sort_by(|a, b| a.cmp(b));
            // let mut sorted_dict = vec![]; // BTreeMap::new();
            // for key in sorted_keys {
            //     if let Some(value) = dict.pop_if(|(k, v)| *k == key) {
            //         sorted_dict.push(value);
            //     }
            // }
            // *dict = sorted_dict;
        }
    }
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

#[cfg(test)]
mod bplist_test {
    use crate::plist::BPlist;
    use std::fs;

    #[test]
    fn test_parse_binary() {
        let data = fs::read("./data/InfoPlist.strings").unwrap();
        let (input, plist) = BPlist::parse(&data).unwrap();
        println!("{:?}", plist)
    }
}
#[cfg(test)]
mod plist_test {
    use crate::plist::Plist;

    #[test]
    fn test_parse() {
        let xml = r#"<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>AppIDName</key>
	<string>ipadump</string>
	<key>ApplicationIdentifierPrefix</key>
	<array>
	<string>Q4J8HDK83K</string>
	</array>
	<key>CreationDate</key>
	<date>2024-08-17T02:24:50Z</date>
	<key>Platform</key>
	<array>
		<string>iOS</string>
		<string>xrOS</string>
		<string>visionOS</string>
	</array>
	<key>IsXcodeManaged</key>
	<false/>
	<key>DeveloperCertificates</key>
	<array>
		<data>MIIFyTCCBLGgAwIBAgIQQ1PQwRY7PCEtVwuLsjLWtTANBgkqhkiG9w0BAQsFADB1MUQwQgYDVQQDDDtBcHBsZSBXb3JsZHdpZGUgRGV2ZWxvcGVyIFJlbGF0aW9ucyBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTELMAkGA1UECwwCRzMxEzARBgNVBAoMCkFwcGxlIEluYy4xCzAJBgNVBAYTAlVTMB4XDTI0MDgxNzAyMTEyMVoXDTI1MDgxNzAyMTEyMFowgY8xGjAYBgoJkiaJk/IsZAEBDApRNEo4SERLODNLMTcwNQYDVQQDDC5BcHBsZSBEaXN0cmlidXRpb246IEh1YW5MYWkgaHVhbmcgKFE0SjhIREs4M0spMRMwEQYDVQQLDApRNEo4SERLODNLMRYwFAYDVQQKDA1IdWFuTGFpIGh1YW5nMQswCQYDVQQGEwJDTjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAOhgAnWvOEZAjxkFYetRAnR6Bw/yKotTXcDSLvi+rtgU81rqiImgpVsyhiVROxbAe7x2KOXg3PaVrgX+Df5VxaBIqZqUJb81BHEviszpAbAXutTU3az2YUn/DqJRxy13sXWedkgFoJbIQ8x22Ia0pBogaa8MQFyEPVMelHzBD/vTpORhG1C2bDCcio4JFvk3D/KfDuVW4mNbgg6yroiNns2xSbODzcD7zu4huHpUgUKlAfc1agI0g2UjcRen8uBn1KzItUgYXmW43CKM+Bt8Uz0Ds1TmEOU2nXMwlw3qM13xYP0YKC8DovZbSReb7xDet/5nMzo/yGUHmWsGOfiumxcCAwEAAaOCAjgwggI0MAwGA1UdEwEB/wQCMAAwHwYDVR0jBBgwFoAUCf7AFZD5r2QKkhK5JihjDJfsp7IwcAYIKwYBBQUHAQEEZDBiMC0GCCsGAQUFBzAChiFodHRwOi8vY2VydHMuYXBwbGUuY29tL3d3ZHJnMy5kZXIwMQYIKwYBBQUHMAGGJWh0dHA6Ly9vY3NwLmFwcGxlLmNvbS9vY3NwMDMtd3dkcmczMDUwggEeBgNVHSAEggEVMIIBETCCAQ0GCSqGSIb3Y2QFATCB/zCBwwYIKwYBBQUHAgIwgbYMgbNSZWxpYW5jZSBvbiB0aGlzIGNlcnRpZmljYXRlIGJ5IGFueSBwYXJ0eSBhc3N1bWVzIGFjY2VwdGFuY2Ugb2YgdGhlIHRoZW4gYXBwbGljYWJsZSBzdGFuZGFyZCB0ZXJtcyBhbmQgY29uZGl0aW9ucyBvZiB1c2UsIGNlcnRpZmljYXRlIHBvbGljeSBhbmQgY2VydGlmaWNhdGlvbiBwcmFjdGljZSBzdGF0ZW1lbnRzLjA3BggrBgEFBQcCARYraHR0cHM6Ly93d3cuYXBwbGUuY29tL2NlcnRpZmljYXRlYXV0aG9yaXR5LzAWBgNVHSUBAf8EDDAKBggrBgEFBQcDAzAdBgNVHQ4EFgQUTNc65ckP8Lt59YSojGJKJAFYR/EwDgYDVR0PAQH/BAQDAgeAMBMGCiqGSIb3Y2QGAQcBAf8EAgUAMBMGCiqGSIb3Y2QGAQQBAf8EAgUAMA0GCSqGSIb3DQEBCwUAA4IBAQCV7+yY3wHpUnaJvAlF+LBAO6RIRFtFWhIWA6Xof52AVNvWQnjPg03/cUM3Lc3HCq17Agd/l2vF7BorfJobZzZkOcdqfmSbbUAIF0bv3XH41xB0GNqAPuQG5i+TMssMDzlv/O7tIWhh9sN6y6vtiqmiG6OABeO/JxN71GmNgCbjPQGeTspGagMmKns70iMAbUhxcZxtXSkl7UCv+AZxQ/AXKAGaS7L/+js5cRjKNpaepHrPMF7YO0CmP1BcG2GisbcVxfDOsFUVGKVbAzIgb58/JcPei828Ue09a3XcxrBTCTcGtJCx73IlxOv1ldTrkY+jNU99TOowGu7PuOqiGYWm</data>
	</array>

	<key>DER-Encoded-Profile</key>
	<data>MIIPmQYJKoZIhvcNAQcCoIIPijCCD4YCAQExDzANBglghkgBZQMEAgEFADCCBVMGCSqGSIb3DQEHAaCCBUQEggVAMYIFPDAMDAdWZXJzaW9uAgEBMBAMClRpbWVUb0xpdmUCAgFsMBMMBE5hbWUMC2lwYWR1bXAuY29tMBMMDklzWGNvZGVNYW5hZ2VkAQEAMBQMCUFwcElETmFtZQwHaXBhZHVtcDAZDAhUZWFtTmFtZQwNSHVhbkxhaSBodWFuZzAdDAxDcmVhdGlvbkRhdGUXDTI0MDgxNzAyMjQ1MFowHgwOVGVhbUlkZW50aWZpZXIwDAwKUTRKOEhESzgzSzAfDA5FeHBpcmF0aW9uRGF0ZRcNMjUwODE3MDIxMTIwWjAgDBdQcm9maWxlRGlzdHJpYnV0aW9uVHlwZQwFQURIT0MwIQwIUGxhdGZvcm0wFQwDaU9TDAR4ck9TDAh2aXNpb25PUzArDBtBcHBsaWNhdGlvbklkZW50aWZpZXJQcmVmaXgwDAwKUTRKOEhESzgzSzAsDARVVUlEDCRkZTExYWRkOS0xNzI2LTQxZGQtYTc2Mi00NTdmMTljOTdhYTIwOwwVRGV2ZWxvcGVyQ2VydGlmaWNhdGVzMCIEIMvjeTwnoeCbjLwvAV4XgH6c/8trlmmna9zejC8+jqyBMIIBXAwSUHJvdmlzaW9uZWREZXZpY2VzMIIBRAwZMDAwMDgxMDEtMDAwOTE1NDAzNDQyMDAxRQwZMDAwMDgxMjAtMDAxNDE1OEUzRTk4MjAxRQwZMDAwMDgxMDMtMDAwRDY1RUExRUQwQzAxRQwZMDAwMDgxMTAtMDAwNjE4REEzQ0MyODAxRQwZMDAwMDgxMTItMDAwNjUwNTQzNDUzQTAxRQwZMDAwMDgxMjAtMDAwQTIxNEMzQUUyMjAxRQwZMDAwMDgwMjAtMDAxQzRENTQyMUYxMDAyRQwZMDAwMDgxMzAtMDAxQTM0MzEzNjYyMDAxQwwZMDAwMDgxMTAtMDAwNjJEMzYzRTkyODAxRQwZMDAwMDgxMDEtMDAwQTQwRUExRTIyMDAxRQwZMDAwMDgxMjAtMDAxNjA0NTkwRTlCQzAxRQwZMDAwMDgwMzAtMDAxRDI0MjIzRTIwODAyRTCCAiAMDEVudGl0bGVtZW50c3CCAg4CAQGwggIHMCwMFmFwcGxpY2F0aW9uLWlkZW50aWZpZXIMElE0SjhIREs4M0suaXBhZHVtcDAdDA9hcHMtZW52aXJvbm1lbnQMCnByb2R1Y3Rpb24wKwwmY29tLmFwcGxlLmRldmVsb3Blci5hc3NvY2lhdGVkLWRvbWFpbnMMASowIgwdY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQBAf8wOAwkY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQuYWNjZXNzMBAMDmhlYWx0aC1yZWNvcmRzMDYMMWNvbS5hcHBsZS5kZXZlbG9wZXIuaGVhbHRoa2l0LmJhY2tncm91bmQtZGVsaXZlcnkBAf8wOAwzY29tLmFwcGxlLmRldmVsb3Blci5oZWFsdGhraXQucmVjYWxpYnJhdGUtZXN0aW1hdGVzAQH/MDEMI2NvbS5hcHBsZS5kZXZlbG9wZXIudGVhbS1pZGVudGlmaWVyDApRNEo4SERLODNLMDgMM2NvbS5hcHBsZS5kZXZlbG9wZXIudXNlcm5vdGlmaWNhdGlvbnMuY29tbXVuaWNhdGlvbgEB/zATDA5nZXQtdGFzay1hbGxvdwEBADA5DBZrZXljaGFpbi1hY2Nlc3MtZ3JvdXBzMB8MDFE0SjhIREs4M0suKgwPY29tLmFwcGxlLnRva2VuoIIIPDCCAkMwggHJoAMCAQICCC3F/IjSxUuVMAoGCCqGSM49BAMDMGcxGzAZBgNVBAMMEkFwcGxlIFJvb3QgQ0EgLSBHMzEmMCQGA1UECwwdQXBwbGUgQ2VydGlmaWNhdGlvbiBBdXRob3JpdHkxEzARBgNVBAoMCkFwcGxlIEluYy4xCzAJBgNVBAYTAlVTMB4XDTE0MDQzMDE4MTkwNloXDTM5MDQzMDE4MTkwNlowZzEbMBkGA1UEAwwSQXBwbGUgUm9vdCBDQSAtIEczMSYwJAYDVQQLDB1BcHBsZSBDZXJ0aWZpY2F0aW9uIEF1dGhvcml0eTETMBEGA1UECgwKQXBwbGUgSW5jLjELMAkGA1UEBhMCVVMwdjAQBgcqhkjOPQIBBgUrgQQAIgNiAASY6S89QHKk7ZMicoETHN0QlfHFo05x3BQW2Q7lpgUqd2R7X04407scRLV/9R+2MmJdyemEW08wTxFaAP1YWAyl9Q8sTQdHE3Xal5eXbzFc7SudeyA7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</data>

	<key>Entitlements</key>
	<dict>

				<key>com.apple.developer.associated-domains</key>
		<string>*</string>

				<key>com.apple.developer.healthkit.recalibrate-estimates</key>
		<true/>

				<key>application-identifier</key>
		<string>Q4J8HDK83K.ipadump</string>

				<key>keychain-access-groups</key>
		<array>
				<string>Q4J8HDK83K.*</string>
				<string>com.apple.token</string>
		</array>

				<key>com.apple.developer.healthkit</key>
		<true/>

				<key>com.apple.developer.healthkit.access</key>
		<array>
				<string>health-records</string>
		</array>

				<key>get-task-allow</key>
		<false/>

				<key>com.apple.developer.team-identifier</key>
		<string>Q4J8HDK83K</string>

				<key>com.apple.developer.usernotifications.communication</key>
		<true/>

				<key>com.apple.developer.healthkit.background-delivery</key>
		<true/>

				<key>aps-environment</key>
		<string>production</string>

	</dict>
	<key>ExpirationDate</key>
	<date>2025-08-17T02:11:20Z</date>
	<key>Name</key>
	<string>ipadump.com</string>
	<key>ProvisionedDevices</key>
	<array>
		<string>00008101-000915403442001E</string>
		<string>00008120-0014158E3E98201E</string>
		<string>00008103-000D65EA1ED0C01E</string>
		<string>00008110-000618DA3CC2801E</string>
		<string>00008112-000650543453A01E</string>
		<string>00008120-000A214C3AE2201E</string>
		<string>00008020-001C4D5421F1002E</string>
		<string>00008130-001A34313662001C</string>
		<string>00008110-00062D363E92801E</string>
		<string>00008101-000A40EA1E22001E</string>
		<string>00008120-001604590E9BC01E</string>
		<string>00008030-001D24223E20802E</string>
	</array>
	<key>TeamIdentifier</key>
	<array>
		<string>Q4J8HDK83K</string>
	</array>
	<key>TeamName</key>
	<string>HuanLai huang</string>
	<key>TimeToLive</key>
	<integer>364</integer>
	<key>UUID</key>
	<string>de11add9-1726-41dd-a762-457f19c97aa2</string>
	<key>Version</key>
	<integer>1</integer>
</dict>
</plist>
    "#;
        let mut value = Plist::parse(xml.as_bytes()).unwrap();
        if let Plist::Dictionary(dict) = &mut value {
            if let Some(Plist::Boolean(value)) = dict.get("hello") {
                assert_eq!(*value, true);
            }
        }
        // value.sort_key();
        println!("{}", value.to_xml());
    }
}
