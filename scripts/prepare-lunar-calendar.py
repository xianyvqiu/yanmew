"""开发期离线生成日历；使用固定 MIT 源码快照，不安装或联网获取依赖。"""
from datetime import date, datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import struct
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[1]
RESOURCES = ROOT / "resources" / "calendar"
REVISION = "448f397c1695cadab3899bf460e0042cab7f0e66"
SOURCE_SHA256 = "1c571579e6c286329eb6eb042d6a0f7142556679f585daac3b2e02fca0a1a53b"
VERSION = "yanmew-calendar-v1"
FIRST, LAST = date(1950, 1, 1), date(2100, 12, 31)
TERMS = "小寒 大寒 立春 雨水 惊蛰 春分 清明 谷雨 立夏 小满 芒种 夏至 小暑 大暑 立秋 处暑 白露 秋分 寒露 霜降 立冬 小雪 大雪 冬至".split()


def generate():
    archive = RESOURCES / "source" / "lunar-python.zip"
    raw = archive.read_bytes()
    if hashlib.sha256(raw).hexdigest() != SOURCE_SHA256:
        raise ValueError("历法源码快照校验失败，不能生成资源")
    with zipfile.ZipFile(archive) as source:
        prefix = f"lunar-python-{REVISION}/"
        license_bytes = source.read(prefix + "LICENSE")
        if license_bytes != (RESOURCES / "LUNAR_PYTHON_LICENSE").read_bytes():
            raise ValueError("源码快照与保留的许可证不一致")
    # 直接从校验过的ZIP加载，避免受全局安装版本影响；无需解包或pip。
    sys.path.insert(0, str(archive) + "/" + prefix.rstrip("/"))
    from lunar_python import LunarYear, Solar

    def solar_date(julian_day):
        solar = Solar.fromJulianDay(julian_day)
        return date(solar.getYear(), solar.getMonth(), solar.getDay())

    months = {}
    terms = bytearray()
    for year in range(1949, 2102):
        lunar_year = LunarYear.fromYear(year)
        for month in lunar_year.getMonths():
            start = solar_date(month.getFirstJulianDay())
            length = month.getDayCount()
            if start > LAST or start + timedelta(days=length) <= FIRST:
                continue
            row = (month.getYear(), abs(month.getMonth()), month.isLeap(), length)
            if length not in (29, 30) or not 1 <= row[1] <= 12:
                raise ValueError("无效农历月份")
            if start in months and months[start] != row:
                raise ValueError("相邻年份计算出不一致月份")
            months[start] = row
        if FIRST.year <= year <= LAST.year:
            # 固定源码协议：索引2..25依次是当年小寒至冬至；0/1属于上年。
            days = lunar_year.getJieQiJulianDays()[2:26]
            if len(days) != len(TERMS):
                raise ValueError("节气数量错误")
            previous = None
            for julian_day in days:
                day = solar_date(julian_day)
                if day.year != year or (previous and day <= previous):
                    raise ValueError("节气年份或顺序错误")
                terms.extend(struct.pack("<H", (day - date(year, 1, 1)).days))
                previous = day
    ordered = sorted(months.items())
    cursor = ordered[0][0]
    data = bytearray()
    lunar_keys = set()
    for start, (year, month, leap, length) in ordered:
        if start != cursor or (year, month, leap) in lunar_keys:
            raise ValueError("农历月份不连续或重复")
        lunar_keys.add((year, month, leap))
        data.extend(struct.pack("<iHBB", (start - FIRST).days, year, month | (0x80 if leap else 0), length))
        cursor = start + timedelta(days=length)
    if not ordered[0][0] <= FIRST or cursor <= LAST:
        raise ValueError("日历范围未完全覆盖")
    outputs = {"months.bin": bytes(data), "solar-terms.bin": bytes(terms)}
    manifest = {
        "resource_version": VERSION,
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "source": {"project": "6tail/lunar-python", "version": "1.4.8", "revision": REVISION,
                   "url": f"https://codeload.github.com/6tail/lunar-python/zip/{REVISION}",
                   "sha256": SOURCE_SHA256, "license": "MIT"},
        "solar_range": [FIRST.isoformat(), LAST.isoformat()],
        "calendar_basis": "中国农历日期；生成器按东八区编算，不按用户时区换算节气时刻",
        "month_count": len(ordered), "day_count": (LAST - FIRST).days + 1,
        "month_record": "8字节，小端：i32公历起日偏移、u16农历年、u8月份(高位为闰月)、u8月长",
        "solar_terms": TERMS,
        "term_record": "1950至2100，每年24个小端u16，表示从公历1月1日起的零基日期偏移",
        "files": {name: {"bytes": len(content), "sha256": hashlib.sha256(content).hexdigest()}
                  for name, content in outputs.items()},
    }
    for name, content in outputs.items():
        (RESOURCES / name).write_bytes(content)
    (RESOURCES / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"生成完成：{manifest['day_count']}天，{len(ordered)}个农历月，{len(TERMS)*151}个节气日期")


if __name__ == "__main__":
    generate()
