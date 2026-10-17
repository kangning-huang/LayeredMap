"""UnivEARTH → sources/univearth.jsonl.

Source: Hugging Face dataset AaronKao/UnivEARTH (licence MIT on the dataset card), revision
1bbeab5f7c90903459069b86d32bf605fdbd8693 (2025-04-23), ``data.json``: 140 yes/no questions about
Earth observation, derived from NASA Earth Observatory articles (Kao et al., arXiv 2504.12110).
The MIT licence allows the questions verbatim (with attribution in LICENSES.md). The extended
ACL-2026 version (IandRover/univearth_arr, MIT) is recorded in the raw data's README, not mapped.

Every question asks about satellite measurements on named dates (water extent, NO2, night
lights, NDVI, burn scars…). LayeredMap has no satellite archive and no internet data, so every
task is out of scope: ``internet_data`` when the answer is a measurement from an archive, and
``imagery`` when it needs reading an image (colours, plumes, what a place looks like).

    python -I tools/lm/benchmarks/convert/univearth.py [/path/to/data.json]
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

from benchlib import CANNOT, case  # noqa: E402

RAW = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("/Volumes/LaCie/data/layeredmap-benchmarks/univearth/hf/data.json")
OUT = HERE.parent / "sources/univearth.jsonl"

IMAGERY = re.compile(r"colou?r|bloom|darker|lighter|brown|milky|plume|solar farm|shrimp|red flowers|island appear|ash from|redness|greener|brightness", re.I)
CHANGE = re.compile(r"increase|decrease|shrink|expand|loss|decline|compared|than|exceed|from .* to|between", re.I)

# Our Chinese translations of every fifth question (for Chinese refusal testing).
ZH = {
    0: "2016年2月11日五大湖上空有云吗？",
    5: "2001年至2014年间柬埔寨是否发生了森林减少？",
    10: "2024年1月15日的卫星影像是否显示德里都市区被云完全覆盖？",
    15: "2023年11月5日到11月21日之间，巴西阿古杜周边5公里内的水面面积是否减少了？",
    20: "2024年10月，澳大利亚的一氧化碳浓度是否高于印度尼西亚？",
    25: "2003年至2019年巴西的耕地面积是否减少？",
    30: "2002年11月到2019年11月，迪古尔河沿岸巴纳梅佩附近的森林覆盖是否减少？",
    35: "2018年7月至8月，印度喀拉拉邦的降雨是否比斯里兰卡少？",
    40: "2020年2月10日至25日，武汉市的二氧化氮浓度与1月1日至20日相比是否上升？",
    45: "2022年8月7日死亡谷国家公园的地表水面积与7月11日相比是否增加？",
    50: "1984年11月至2022年11月，都柏林的建成区面积是否减少？",
    55: "2021年9月29日，龟山岛东南岸的海水颜色是否比西北岸更浅？",
    60: "2021年2月20日至3月3日，五大湖的冰盖面积是否增加？",
    65: "2016年1月17日至24日，台北的地表温度是否低于同期2001—2010年的平均值？",
    70: "2024年，布莱斯峡谷国家公园的夜间灯光强度是否高于拉斯维加斯？",
    75: "2020年1月至7月22日，巴拉那河三角洲的火点数是否超过2019年同期？",
    80: "2021年11月，东热带太平洋的海表温度是否高于11月的长期平均值？",
    85: "2017年4月加利福尼亚的土壤湿度是否低于2012年8月？",
    90: "2020年2月2日，密西西比河下游的地表水范围与2019年12月4日相比是否增加？",
    95: "2022年10月23日，乍得迪甘加利的地表水范围与2021年9月18日相比是否减少？",
    100: "2002年西伯利亚北部塔兹河以东的湖泊数量是否比1973年少？",
    105: "2018年10月22日，上箭湖地区的山谷雾从早上到下午是否增加？",
    110: "2016年8月土耳其图兹湖的水面面积是否比1988年8月大？",
    115: "2020年2月23日至4月29日，和田附近塔克拉玛干沙漠的植被覆盖是否增加？",
    120: "根据MODIS地表温度数据，2004年美国死亡谷是否记录到全球最高的白天地表温度？",
    125: "2025年1月7日下午，洛杉矶太平洋帕利塞兹以西50公里区域的对流层二氧化氮浓度是否高于其西南50公里区域？",
    130: "2013年1月加州Coastal Sage UCI站点的土壤湿度是否高于2013年5月？",
    135: "2019年巴西的火点探测数是否少于2007年？",
}


def main() -> int:
    data = json.loads(RAW.read_text(encoding="utf-8"))
    records = []
    for i, x in enumerate(data):
        q = x["question"].strip()
        imagery = x["tag"] == "Water Color" or bool(IMAGERY.search(q))
        reason = "imagery" if imagery else "internet_data"
        category = "imagery" if imagery else ("temporal" if CHANGE.search(q) else "data_acquisition")
        cases = [case(f"uve-{i:03d}", q, CANNOT, context="world_plus")]
        if i in ZH:
            cases.append(case(f"uve-{i:03d}-zh", ZH[i], CANNOT, context="world_plus", lang="zh"))
        records.append(
            {
                "source": "UnivEARTH",
                "source_id": f"{i:03d}",
                "licence": "MIT",
                "text_policy": "verbatim",
                "summary": f"Yes/no Earth-observation question ({x['tag']}): "
                + ("interpret satellite imagery for a named place and date" if imagery else "compare a satellite-archive measurement for named places or dates"),
                "category": category,
                "coverage": "out_of_scope",
                "oos_reason": reason,
                "confidence": "high",
                "notes": "Needs satellite data for the dates named, which no project layer holds. If the user had loaded such rasters, summarize_here could compare two areas, but the request names none.",
                "cases": cases,
            }
        )
    OUT.write_text("".join(json.dumps(r, ensure_ascii=False) + "\n" for r in records), encoding="utf-8")
    print(OUT.name, len(records), "records,", sum(len(r["cases"]) for r in records), "cases")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
