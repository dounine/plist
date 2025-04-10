use std::fs;
use plist::plist::PlistValue;

fn main() {
    let data = fs::read("./data/Info.plist").unwrap();
    let plist = PlistValue::parse(&data).unwrap();
    println!("{:?}", plist)
}
