#!/bin/bash

# 自动转换 plist 格式脚本
# 用法: ./plist_convert.sh filename.plist

# 检查参数
if [ $# -ne 1 ]; then
  echo "Usage: $0 <plist-file>"
  exit
fi

target_file="$1"

# 检查文件是否存在
if [ ! -f "$target_file" ]; then
  echo "Error: File $target_file not found!"
  exit 2
fi

# 检测文件类型
file_type=$(file --brief $target_file)

## 判断并执行转换
if echo "$file_type" | grep -q "Apple binary property list"; then
  echo "Detected binary plist, converting to XML..."
  plutil -convert xml1 $target_file
elif echo "$file_type" | grep -q "XML 1.0 document text"; then
  echo "Detected XML plist, converting to binary..."
  plutil -convert binary1 -s -r $target_file
else
  echo "Error: Unsupported file format"
  echo "Detected type: $file_type"
  exit 3
fi

echo "Conversion complete!"