# librepods-linux-backup

LibrePods Linux 个人修改版源码备份

基于官方 LibrePods 项目，沿用上游许可证，见 LICENSE，上游自述文件保留为 README.upstream.md

官方项目：https://github.com/librepods-org/librepods

技术文档另行保存：https://github.com/csjcsl666/software-dev-notes/tree/main/librepods

## 查看当前蓝牙音频编码

```bash
pw-dump | jq -r '.[] | select(.type == "PipeWire:Interface:Node") | .info.props | select(."api.bluez5.codec" != null) | .["api.bluez5.codec"]'
```

- `aac`：AAC
- `sbc_xq`：SBC-XQ
- `sbc`：SBC

没有输出表示未从当前 PipeWire 节点读取到相应信息，不要直接解释为设备不支持
