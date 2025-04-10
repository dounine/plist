use std::fs;
use plist::plist::Plist;

fn main() {
    let data = fs::read("./data/Info.plist").unwrap();
    let plist = Plist::parse(&data).unwrap();
    println!("{:?}", plist)
}
