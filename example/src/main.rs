use std::fs;
use plist::plist::PlistValue;

fn main() {
    let data = fs::read("./data/InfoPlist.strings").unwrap();
    let plist = PlistValue::parse(&data).unwrap();
    println!("{:?}", plist)
}
