use plist::plist::Plist;
use std::fs;

fn main() {

    let count = 15;
    let object_type = 0x5 & 0x0F; // 高4位掩码
    let extra_info = count & 0x0F; // 低4位掩码
    // 合并字节：object_type << 4 | extra_info
    let header_byte = ((object_type << 4) as usize | extra_info) as u8;

    let data = fs::read("./data/Info.plist").unwrap();
    let plist = Plist::parse(&data).unwrap();
    let binary_bytes = plist.to_binary().unwrap();
    fs::write("./data/copy.plist", &binary_bytes).unwrap();
    println!("{:?}", plist)
}
