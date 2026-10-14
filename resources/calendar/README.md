# 颜喵离线农历与节气资源

资源版本 `yanmew-calendar-v1`，支持公历 1950-01-01 至 2100-12-31。数据在开发阶段由 `6tail/lunar-python` 1.4.8 固定提交 `448f397c1695cadab3899bf460e0042cab7f0e66` 生成。上游作者 6tail，许可证为 MIT，原文保留在 `LUNAR_PYTHON_LICENSE`。原始源码 ZIP 和 SHA-256 保存在 `source/` 与 `manifest.json`，项目代码的 Apache-2.0 不替代该资源的上游 MIT 声明。

- 源码：[固定提交](https://github.com/6tail/lunar-python/tree/448f397c1695cadab3899bf460e0042cab7f0e66)。源码快照获取日期为 2026-10-13。
- `months.bin`：1,869 条、每条 8 字节，共 14,952 字节。小端 i32 公历起日相对 1950-01-01 的偏移，u16 农历年，u8 月份（最高位标记闰月），u8 月长。按日期、农历年/月/闰月均升序。边缘月份保留完整月长，查询结果仍限制在支持的公历范围。
- `solar-terms.bin`：151 年 × 24 条 × 2 字节，共 7,248 字节。每条为小端 u16、从当年 1 月 1 日起的零基日偏移。顺序为小寒至冬至，见 manifest。
- `manifest.json`：生成时间、范围、版本、固定源码与二进制 SHA-256、编码格式。
- `reference-metadata.json`：独立核对所用官方文件的链接、获取时间及校验和；只记录来源元数据，不分发官方年度文本。固定期望在 `lunar.rs` 测试中，不由生成器自动填写。

运行时 `lunar.rs` 通过 `include_bytes!` 将二进制表编译进程序，不读取外部可变历表、不联网、不调用 Python。查询为中国农历日期口径；输入是日期而非 UTC 时刻，提醒仍由原本机时区调度。未提供的闰月、超出月长的日期和范围外结果返回 None。

在项目根目录执行 `python -B scripts/prepare-lunar-calendar.py` 可离线重建；脚本只使用随仓库保存且校验过的源码快照，不安装依赖。二进制可重复生成，manifest 的生成时间随执行更新。更新源码或算法必须更换资源版本、校验和并重新核对固定日期；不能用另一套算法零散修补个别年份。

香港天文台公开对照表仅用于独立样本核对：[日历页面](https://www.hko.gov.hk/en/gts/time/conversion.htm)。本次核对春节、中秋、立春、清明、冬至、2023 闰二月、2033 闰十一月、2100 边界及年初上一农历年。不能据此宣称所有远期日期都与官方预测逐日相同；远期临近午夜的天文日期可能在资源升级时修订。

便携版执行 `scripts/copy-calendar-notices.ps1 -Destination <便携版目录>`，复制来源说明、manifest 和 MIT 许可证到 exe 旁 `resources/calendar/`。源码 ZIP 是开发复现材料，不必随用户程序分发；用户备份不导出日历表、源码和许可证文件。
