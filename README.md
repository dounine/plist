# plist

#bplist00转xml
```shell
plutil -convert xml1 ./data/InfoPlist.strings
```
#xml转bplist00
```shell
plutil -convert binary1 -s -r ./data/InfoPlist.strings
```