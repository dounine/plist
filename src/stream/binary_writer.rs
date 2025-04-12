use crate::error::Error;
use crate::plist::Plist;
use chrono::{DateTime, Utc};
use std::io::{Cursor, Write};

pub(crate) struct BinaryWriter {
    objects: u64,
    offsets: Vec<u64>, // 每个对象的偏移量
    ref_size: u8,      // 对象引用大小 (1/2/4/8字节)
    offset_size: u8,   // 偏移表条目大小 (1/2/4/8字节)
}
impl BinaryWriter {
    pub fn new() -> Self {
        BinaryWriter {
            objects: 0,
            // object_data: vec![],
            offsets: vec![],
            ref_size: 1,
            offset_size: 1,
        }
    }

    pub fn write<W: Write>(mut self, value: &Plist, output: &mut W) -> Result<(), Error> {
        // 1. 收集所有对象并生成二进制数据
        let mut bytes = vec![];
        let (objects_data, _) = self.collect_objects(value, &mut bytes)?;
        //2. 写入头部
        output.write_all(b"bplist00")?;
        //3. 写入偏移表
        let mut cursor = Cursor::new(vec![]);
        for (_, data) in objects_data.iter().enumerate() {
            self.offsets.push(cursor.position() + 8);
            cursor.write_all(data)?;
        }
        let object_bytes = cursor.into_inner();
        let offset_table_start = object_bytes.len() + 8;
        output.write_all(&object_bytes)?;
        //4. 计算元数据
        self.calculate_sizes();
        //5. 写入偏移表
        let offset_table = self.generate_offset_table()?;
        output.write_all(&offset_table)?;
        // 6. 写入尾部
        let trailer_table = self.generate_trailer(0, bytes.len(), offset_table_start as u64)?;
        output.write_all(&trailer_table)?;
        Ok(())
    }

    fn collect_objects<'a>(
        &mut self,
        value: &Plist,
        mem_bytes: &'a mut Vec<(u64, Vec<Vec<u8>>)>,
    ) -> Result<(Vec<Vec<u8>>, Vec<u8>), Error> {
        let index = self.objects;
        self.objects += 1;
        let bytes = self.serialize_object(value, mem_bytes)?;
        let exit_bytes = mem_bytes.iter().find(|(_, d)| **d == bytes);
        let (bytes, index) = if let Some((key_idx, _)) = exit_bytes {
            self.objects -= 1;
            (vec![], *key_idx)
        } else {
            mem_bytes.push((index, bytes.clone()));
            (bytes, index)
        };
        Ok((bytes, self.serialize_ref(index)))
    }
    fn serialize_object<'a>(
        &mut self,
        value: &Plist,
        mem_bytes: &'a mut Vec<(u64, Vec<Vec<u8>>)>,
    ) -> Result<Vec<Vec<u8>>, Error> {
        let mut list = vec![];
        match value {
            Plist::Array(value) => {
                let mut buffer = vec![];
                let (marker, len_bytes) = self.serialize_length(0xA, value.len());
                buffer.push(marker);
                buffer.extend(len_bytes);
                let mut datas = vec![];
                for elem in value {
                    let (data, ref_bytes) = self.collect_objects(elem, mem_bytes)?;
                    buffer.extend(ref_bytes);
                    datas.extend(data);
                }
                list.push(buffer);
                list.extend(datas);
            }
            Plist::Dictionary(dict) => {
                let mut buffer = vec![];
                let (marker, len_bytes) = self.serialize_length(0xD, dict.len());
                buffer.push(marker);
                buffer.extend(len_bytes);
                let mut datas = vec![];
                for (key, _) in dict {
                    let key_plist = Plist::String(key.clone());
                    let (data, ref_bytes) = self.collect_objects(&key_plist, mem_bytes)?;
                    buffer.extend(ref_bytes);
                    datas.extend(data);
                }
                for (_, value) in dict {
                    let (data, ref_bytes) = self.collect_objects(value, mem_bytes)?;
                    buffer.extend(ref_bytes);
                    datas.extend(data);
                }
                list.push(buffer);
                list.extend(datas);
            }
            Plist::Boolean(value) => {
                let mut buffer = vec![];
                let marker = if *value { 0x09 } else { 0x08 };
                buffer.push(marker);
                list.push(buffer);
            }
            Plist::Integer(value) => {
                let mut buffer = vec![];
                let (marker, bytes) = self.serialize_integer(0x1, *value);
                buffer.push(marker);
                buffer.extend(bytes);
                list.push(buffer);
            }
            Plist::Float(value) => {
                let mut buffer = vec![];
                let (marker, bytes) = self.serialize_float(0x2, *value);
                buffer.push(marker);
                buffer.extend(bytes);
                list.push(buffer);
            }
            Plist::String(value) => {
                let mut buffer = vec![];
                let bytes = value.as_bytes();
                let (marker, len_bytes) = self.serialize_length(0x5, bytes.len());
                buffer.push(marker);
                buffer.extend(len_bytes);
                buffer.extend(bytes);
                list.push(buffer);
            }
            Plist::Date(value) => {
                let mut buffer = vec![];
                let (marker, bytes) = self.serialize_date(0x3, *value);
                buffer.push(marker);
                buffer.extend(bytes);
                list.push(buffer);
            }
            Plist::Data(value) => {
                let mut buffer = vec![];
                let (marker, bytes) = self.serialize_data(0x4, value);
                buffer.push(marker);
                buffer.extend(bytes);
                buffer.extend(value);
                list.push(buffer);
            }
        }
        Ok(list)
    }
    fn generate_trailer(
        &self,
        root_index: usize,
        objects: usize,
        offset_table_start: u64,
    ) -> Result<Vec<u8>, Error> {
        let mut trailer = [0_u8; 32];
        //未使用区域(6字节)
        trailer[6] = self.offset_size;
        trailer[7] = self.ref_size;
        // let num_object = self.objects;
        trailer[8..16].copy_from_slice(&objects.to_be_bytes());
        trailer[16..24].copy_from_slice(&(root_index as u64).to_be_bytes());
        trailer[24..32].copy_from_slice(&offset_table_start.to_be_bytes());
        Ok(trailer.to_vec())
    }
    fn generate_offset_table(&self) -> Result<Vec<u8>, Error> {
        let mut table = vec![];
        for offset in &self.offsets {
            match self.offset_size {
                1 => table.push(*offset as u8),
                2 => table.extend(&(*offset as u16).to_be_bytes()),
                4 => table.extend(&(*offset as u32).to_be_bytes()),
                8 => table.extend(&(*offset).to_be_bytes()),
                _ => return Err(Error::Error("Invalid offset size".to_string())),
            }
        }
        Ok(table)
    }
    fn serialize_ref(&self, index: u64) -> Vec<u8> {
        match self.ref_size {
            1 => vec![index as u8],
            2 => (index as u16).to_be_bytes().to_vec(),
            4 => (index as u32).to_be_bytes().to_vec(),
            8 => index.to_be_bytes().to_vec(),
            _ => panic!("Invalid ref size"),
        }
    }
    fn serialize_length(&self, code: u8, len: usize) -> (u8, Vec<u8>) {
        let object_type = code & 0x0F; // 高4位掩码
        let extra_info = len & 0x0F; // 低4位掩码
        // 合并字节：object_type << 4 | extra_info
        if len < 0xF {
            let header_byte = ((object_type << 4) as usize | extra_info) as u8;
            (header_byte, vec![])
        } else {
            let header_byte = code << 4 | 0x0F;
            let size_bytes = self.serialize_count(len);
            (header_byte, size_bytes)
        }
    }
    fn serialize_count(&self, count: usize) -> Vec<u8> {
        let bytes_needed: i32 = match count {
            0..=0xFF => 1,
            0x100..=0xFFFF => 2,
            0x10000..=0xFFFFFFFF => 4,
            _ => 8,
        };
        let type_byte = (bytes_needed << 4) as u8;
        let mut bytes = match bytes_needed {
            1 => vec![count as u8],
            2 => (count as u16).to_be_bytes().to_vec(),
            4 => (count as u32).to_be_bytes().to_vec(),
            8 => (count as u64).to_be_bytes().to_vec(),
            _ => panic!("Invalid count"),
        };
        bytes.insert(0, type_byte);
        bytes
    }
    fn serialize_data(&self, code: u8, value: &Vec<u8>) -> (u8, Vec<u8>) {
        let len = value.len();
        if len < 0xF {
            (code | len as u8, vec![])
        } else {
            let len_bytes = self.serialize_count(len);
            (code | 0x0F, len_bytes)
        }
    }
    fn serialize_date(&self, code: u8, value: DateTime<Utc>) -> (u8, Vec<u8>) {
        let unix_timestamp = value.timestamp() as f64 + value.timestamp_subsec_nanos() as f64 / 1e9;
        let seconds_since_2001 = unix_timestamp - 978_307_200.0;
        (code << 4 | 3, seconds_since_2001.to_be_bytes().to_vec())
    }
    fn serialize_float(&self, code: u8, value: f64) -> (u8, Vec<u8>) {
        let as_f32 = value as f32;
        let is_lossless = (as_f32 as f64) == value;
        let (extra_info, bytes) = if is_lossless && (value == 0.0 || value.abs() <= f32::MAX as f64)
        {
            // 使用 32-bit 浮点数（无精度丢失）
            (0x0, as_f32.to_be_bytes().to_vec())
        } else {
            // 必须使用 64-bit 浮点数
            (0x3, value.to_be_bytes().to_vec())
        };
        ((code << 4) | (extra_info & 0x0F), bytes)
    }
    fn serialize_integer(&self, code: u8, value: i64) -> (u8, Vec<u8>) {
        let code = code << 4;
        let (extra_info, bytes) = if value >= 0 {
            match value {
                0..=0xFF => (0x0, vec![value as u8]),
                0x100..=0xFFFF => (0x1, (value as u16).to_be_bytes().to_vec()),
                0x10000..=0xFFFFFFFF => (0x2, (value as u32).to_be_bytes().to_vec()),
                _ => (0x3, value.to_be_bytes().to_vec()),
            }
        } else {
            panic!("Negative integers not implemented");
        };
        (code | (extra_info & 0x0F), bytes)
    }

    fn calculate_sizes(&mut self) {
        let max_ref = self.objects;
        self.ref_size = if max_ref <= 0xFF {
            1
        } else if max_ref <= 0xFFFF {
            2
        } else if max_ref <= 0xFFFFFFFF {
            4
        } else {
            8
        };
        let max_offset = *self.offsets.last().unwrap_or(&0);
        self.offset_size = if max_offset <= 0xFF {
            1
        } else if max_offset <= 0xFFFF {
            2
        } else if max_offset <= 0xFFFFFFFF {
            4
        } else {
            8
        };
    }
}
