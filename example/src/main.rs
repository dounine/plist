use plist::plist::Plist;
use std::fs;

fn main() {
    let data = fs::read("./data/Info.plist").unwrap();
    let plist = Plist::parse(&data).unwrap();
    let binary_bytes = plist.to_binary().unwrap();
    fs::write("./data/copy.plist", &binary_bytes).unwrap();
    println!("{:?}", plist)
}
