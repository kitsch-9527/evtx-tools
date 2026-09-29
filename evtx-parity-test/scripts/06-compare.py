"""06-compare.py v5 — 基线解析 vs 新解析归一化比对。

v5（2026-09-29）：G0 基线语言探测。Lua 基线（wevtutil /f:text）渲染语言跟随本机 UI 语言，
主比对语言必须与基线一致，否则全量 D 类假阳性（本机 UI 语言 en→zh 翻转实测 matched
32020→1840）。现从基线文本探测语言（zh：'Level: 信息'；en：'Level: Information'），
主比对 = 基线 vs new-parsed/同语言，副侧只计覆盖率。report.json 新增 baseline_lang；
覆盖率统计键由 zh_* 改名 cov_*。

阶段B（2026-09-09，ADR 0006）：内置豁免白名单（EXEMPTIONS，键=通道+provider+eid，语言无关）。
默认口径：白名单命中的 D 类 diff 计入 exempted（report.json 新增 exempted/exempted_detail/
whitelist_diff_hits）。--no-exemptions：白名单仍按 D 计入 diff（G6 关豁免复核口径）。"""
from __future__ import annotations
import argparse
import csv
import re
import sys
from datetime import datetime, timedelta
from pathlib import Path
TEST_ROOT = Path(__file__).resolve().parent.parent
BASELINE_DIR = TEST_ROOT / "baseline-parsed"
NEW_ZH_DIR = TEST_ROOT / "new-parsed" / "zh"
NEW_EN_DIR = TEST_ROOT / "new-parsed" / "en"
CHANNELS = ["Application", "Setup", "Security", "System"]
TIME_TOLERANCE = timedelta(seconds=2)
TOP_N = 20
WS_RE = re.compile(r"\s+")
EVENT_SPLIT_RE = re.compile(r"Event\[\d+\]")
# TRAIL_RE 剥 baseline 块尾的管线残留：';'（tr 压行分隔）与 ':'（sed 把消息尾 NUL 换成 ':'，
# 如 "...Arbiter.:;;;;" 的 ':' 来自原文末尾 \x00，非消息语义——实测 raw 文本以 \x00 结尾）。
# 曾尝试保留 ':'（加固 3），导致基线尾部多一 ':' 而全量失配（matched=0），已回退。
TRAIL_RE = re.compile(r"[;\s]+$")
# R7（2026-09-04）：TRAIL 不再含 ':'。sed 把消息尾 NUL 换成 ':'（形态 `XXX:;;;;`），与模板尾真冒号
# （SCM7045 `Service Account:` + 空param，live 取证）混杂。剥离分两步：先去 sed 冒号段
# （尾 `:` + 其后仅 ;/空白），再剥 ;/空白。模板尾冒号保留。
# 阶段B（2026-09-09）ADR 0006 豁免白名单：键 = 通道 + provider + event_id，值为根因码。
# 豁免判定前移进比对器：默认口径下命中键的 D 类 diff 条目移出 diffs 计入 exempted
# （report.json 新增 exempted / exempted_detail / whitelist_diff_hits 字段）；
# --no-exemptions（G6 复核口径）下白名单仍按 D 计入 diff。
# 本表必须与 scripts/acceptance-check.py 的 EXEMPTIONS 表内容一致，两处改动须同一提交；
# 登记表唯一真源：docs/residual-zeroing-plan-2026-09-07.md §4。
EXEMPTIONS = {
    "entries": [
        ("Application", "Microsoft-Windows-CAPI2", "4111", "PARSE"),
        ("System", "Microsoft-Windows-Directory-Services-SAM", "16962", "BIN"),
        ("System", "Microsoft-Windows-WPDClassInstaller", "24577", "PARSE"),
        ("System", "Microsoft-Windows-WPDClassInstaller", "24579", "PARSE"),
    ],
}
EXEMPTION_KEYS = {(ch, prov, eid): code for ch, prov, eid, code in EXEMPTIONS["entries"]}
def norm_text(s):
    if not s:
        return ""
    s = s.replace("\r\n", "\n").replace("\r", "\n")
    return WS_RE.sub(" ", s).strip()
def clean_baseline_artefacts(s):
    # R5（2026-09-04）：基线管线 artefact 手术刀清洗。iconv 对 UTF-16 残留/非法字节输出 '?' 替换符
    # （System EID 12 时间串 "?2026?-?08?-?20T21:…"，live479 条），但参数值里的 '?' 是 wevtapi
    # 真实渲染（live 取证：EID16 \??\ 前缀、SPP1003 [(?)(?)]、WER 中文 Response ?? 均原样存在），
    # 全局删 '?' 会吃掉语义。只删时间串形态 "?YYYY?-?MM?-?DDT"，其余一律保留。
    s = re.sub(r"\?(\d{4})\?-\?(\d{2})\?-\?(\d{2}T)", r"\1-\2-\3", s)
    return s


def norm_msg_time_frac(s):
    # 时间小数位豁免（2026-09-04 决定不修）：基线管线保留 9 位小数（.500000000Z），
    # 新解析 chrono %6f 截到 6 位（.500000Z）。语义相等，比对时统一截到 6 位。
    return re.sub(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{6})\d*(Z?)", r"\1\2", s)


def detect_baseline_lang(path):
    # G0（v5）：wevtutil /f:text 的字段标签恒为英文（Log Name/Event ID/Date/Level），
    # 字段值本地化。zh 基线 'Level: 信息'，en 基线 'Level: Information'。
    # 主比对语言随此探测值走；探测失败必须 BLOCKED，不得静默选错语言（09-29 事故教训）。
    try:
        raw = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return "unknown"
    if "Level: 信息" in raw:
        return "zh-CN"
    if "Level: Information" in raw:
        return "en-US"
    return "unknown"
def parse_baseline(path):
    raw = path.read_text(encoding="utf-8", errors="replace").replace("\x00", "")
    raw = clean_baseline_artefacts(raw)
    starts = [m.start() for m in EVENT_SPLIT_RE.finditer(raw)]
    if not starts:
        return []
    events = []
    for i, st in enumerate(starts):
        ed = starts[i + 1] if i + 1 < len(starts) else len(raw)
        blk = raw[st:ed]
        eid = re.search(r"Event ID:\s*(\d+)", blk)
        prov = re.search(r"Source:\s*(.*?)\s*;;", blk)
        logn = re.search(r"Log Name:\s*(.*?)\s*;;", blk)
        date = re.search(r"Date:\s*(\S+)", blk)
        m = re.search(r"Description:\s*(.*)$", blk, re.DOTALL)
        desc = m.group(1) if m else ""
        # R5（2026-09-04）：分号三段论——前导 ;; 是字段分隔残留（Description: 后换行被 tr 压成 ;，
        # 全文件 3925/3925 块均有），块尾 ;;;; 是压行残留，两者剥除；消息中间分号是参数值本身
        # （live 取证 SPP8230/8198 参数值分号分隔），一律保留。比较时双向容忍见 canon_msg。
        desc = re.sub(r"^[;\s]+", "", desc.strip())
        desc = re.sub(r":(?=[;\s]*$)", "", desc)
        desc = TRAIL_RE.sub("", desc)
        draw = date.group(1).strip() if date else ""
        draw = TRAIL_RE.sub("", draw)
        events.append({
            "event_id": eid.group(1) if eid else "",
            "provider": norm_text(prov.group(1) if prov else ""),
            "channel": norm_text(logn.group(1) if logn else ""),
            "date_utc": draw,
            "message": norm_text(desc),
        })
    return events
def parse_baseline_time(s):
    # 基线 Date 看似 UTC（带 Z）实为本地时间：wevtutil /f:text 渲染本地时间但保留 Z 后缀
    # （与 raw XML SystemTime 相差 +8h，Setup#0/EventRecordID=1 实测证实），故按本地 naive 解析。
    # 小数秒被截断补零（.6720013→.6720000），统一补齐 6 位后与新解析（本地微秒）直接比。
    if not s:
        return None
    t = s.strip()
    if t.endswith("Z"):
        t = t[:-1]
    m = re.match(r"(.*\.)(\d+)$", t)
    if m:
        t = m.group(1) + (m.group(2) + "000000")[:6]
    for fmt in ("%Y-%m-%dT%H:%M:%S.%f", "%Y-%m-%dT%H:%M:%S"):
        try:
            return datetime.strptime(t, fmt)
        except ValueError:
            continue
    return None
def parse_local(s):
    if not s:
        return None
    for fmt in ("%Y-%m-%d %H:%M:%S.%f", "%Y-%m-%d %H:%M:%S"):
        try:
            return datetime.strptime(s.strip(), fmt)
        except ValueError:
            continue
    return None
def load_new_csv(path):
    with path.open(encoding="utf-8-sig", newline="") as f:
        all_rows = list(csv.DictReader(f))
        # 加固 1：表头重复行是上游 bug（write_events 每 batch 重写 BOM+头），此处过滤并计数；
        # event_id 为空的合法行理论上不应存在，若出现会计入 dropped_empty 并在报告中可见。
        rows = []
        dropped_header = 0
        dropped_empty = 0
        for r in all_rows:
            eid = (r.get("event_id") or "").strip()
            if eid.isdigit():
                rows.append(r)
            elif eid in ("event_id", ""):
                if eid == "":
                    dropped_empty += 1
                else:
                    dropped_header += 1
            else:
                dropped_empty += 1
        load_new_csv.last_dropped = (dropped_header, dropped_empty)
    def seq(r):
        try:
            return int(r.get("sequence_number") or 0)
        except ValueError:
            return 0
    rows.sort(key=seq)
    out = []
    for r in rows:
        out.append({
            "event_id": (r.get("event_id") or "").strip(),
            "provider": norm_text(r.get("provider_name")),
            "channel": norm_text(r.get("channel")),
            "event_time_local": (r.get("event_time") or "").strip(),
            "message": norm_text(r.get("message")),
        })
    return out
def canon_msg(s):
    # R6（2026-09-04）：管线对称化。基线管线 tr 把消息内换行压成 ';'，与参数值真分号混杂
    # （live 取证：SPP8230 新解析分号与 wevtapi live 一致，基线侧分号已被管线改写，信息丢失）。
    # 比较时双向 '；'→' '（仅比较，不改上游渲染），换行差异与参数分号同时对齐。
    # hex 大小写 / GUID 花括号 / '?' 仍严格，上游必须完全一致。
    # 阶段A（2026-09-08）WER `\\?\` 归一：双向等价 `\\?\` ≡ `\\\`。计划依据：R5 live wevtapi
    # 字节取证（skill 基线 #4/#5）——渲染器输出正确，基线管线吃掉 '?'。
    # 实测复核：当前 799 diff 中 WER 1001 仅 2 条且两侧 `\\?\` 均完整（首差异在 GUID 尾段
    # `3003916586` vs `0xb30c212a`，属上游 hex 化问题），本规则当前影响面=0（保留规则待上游修复后生效）。
    s = s.replace("\\\\?\\", "\\\\\\")
    # 阶段A（2026-09-08）CAPI2 LRM 归一：剥离 U+200E/200F（方向标记）后比对。计划依据：基线
    # 管线把 LRM 转成 '?'（CAPI2 4111 `?2026?年?8?月?25?日` vs 我方真实 LRM，影响面 1 条）。
    # 实测复核：仅剥 LRM 后基线侧 '?' 仍在，1 条不落；彻底清除需基线侧 '?' 手术（R5 同层，
    # 超出本阶段授权），已按退出条件登记 blocked。规则保留（方向标记无语义，剥离无损）。
    s = s.replace("\u200e", "").replace("\u200f", "")
    # R21（2026-09-29）帐户/账户 归一：SCM 7045 等模板的 zh 译文在磁盘资源
    # （System32\zh-CN\services.exe.mui——全 System32 唯一模板载体，字节级实证）为"帐户"，
    # 而 wevtapi 实际渲染"账户"（111/111 实测）。账户版文件不存在于文件系统（疑似
    # LOS/增量服务缓存），萃取侧不可达。按 CAPI2 `?` 先例登记为比对层等价归一
    # （一字级异体词，无语义差），上游渲染保持原样不受影响。
    s = s.replace("帐户", "账户")
    return WS_RE.sub(" ", s.replace(";", " ")).strip()


def classify_msg_diff(bm, zm):
    if "cannot be found" in zm:
        return "C-无模板fallback"
    # B 类阈值来源——实测裸参数行最长约 120 字（如 System 6009 的 "10.00.;;26220;;;;…"），
    # 200 为安全边际；超过 200 且含 ;; 的长消息走严格比较，不误判。
    if ";;" in zm and len(zm) < 200:
        return "B-裸参数未渲染"
    if canon_msg(bm) == canon_msg(zm):
        return "A-渲染格式一致"
    return "D-实质分歧（含渲染格式差：hex大小写/GUID花括号/SDDL分号/?替换符）"


def cmp_pair(b, z):
    problems = []
    if b["event_id"] != z["event_id"]:
        problems.append("EventID 基线=" + b["event_id"] + " 新=" + z["event_id"])
    if b["provider"] != z["provider"]:
        problems.append("Provider 基线=" + b["provider"] + " 新=" + z["provider"])
    if b.get("channel") and z.get("channel") and b["channel"] != z["channel"]:
        problems.append("Channel 基线=" + b["channel"] + " 新=" + z["channel"])
    bt, zt = parse_baseline_time(b["date_utc"]), parse_local(z["event_time_local"])
    if bt is not None and zt is not None:
        if abs(bt - zt) > TIME_TOLERANCE:
            problems.append("时间 基线=" + str(bt) + " 新=" + str(zt))
    elif (bt is None) != (zt is None):
        problems.append("时间一方缺失 基线=" + b["date_utc"] + " 新=" + z["event_time_local"])
    bm_cmp = norm_msg_time_frac(b["message"])
    zm_cmp = norm_msg_time_frac(z["message"])
    if bm_cmp != zm_cmp:
        # 严格口径：空白归一 + 时间小数位豁免后完全相等才算一致。
        cls = classify_msg_diff(bm_cmp, zm_cmp)
        if cls == "A-渲染格式一致":
            pass
        else:
            problems.append(cls + "（见明细）")
    return problems
def compare_channel(channel, primary_lang):
    base_file = BASELINE_DIR / (channel + ".evtx.txt")
    zh_file = NEW_ZH_DIR / (channel + ".csv")
    en_file = NEW_EN_DIR / (channel + ".csv")
    res = {"channel": channel, "status": "ok", "primary_lang": primary_lang}
    if not base_file.exists():
        res["status"] = "缺少基线文件" + base_file.name + "（先跑02）"
        return res
    if primary_lang == "zh-CN" and not zh_file.exists():
        res["status"] = "缺少新解析zh " + str(zh_file) + "（先跑05）"
        return res
    if primary_lang == "en-US" and not en_file.exists():
        res["status"] = "缺少新解析en " + str(en_file) + "（先跑05）"
        return res
    if primary_lang not in ("zh-CN", "en-US"):
        res["status"] = "G0 无法识别基线渲染语言（只支持 zh-CN/en-US），先人工核 02 产物"
        return res
    base = parse_baseline(base_file)
    en = load_new_csv(en_file) if en_file.exists() else []
    zh = load_new_csv(zh_file) if zh_file.exists() else []
    if primary_lang == "zh-CN":
        primary, coverage = zh, en
    else:
        primary, coverage = en, zh
    res["baseline_count"] = len(base)
    res["new_en_count"] = len(en)
    res["new_zh_count"] = len(zh)
    res["zh_empty_msg"] = sum(1 for r in zh if not r["message"])
    res["en_empty_msg"] = sum(1 for r in en if not r["message"])
    n = max(len(base), len(primary))
    matched = 0
    diffs = []
    for i in range(n):
        b = base[i] if i < len(base) else None
        z = primary[i] if i < len(primary) else None
        if b is None or z is None:
            diffs.append({"index": i, "kind": "缺失", "baseline": b, "new": z})
            continue
        problems = cmp_pair(b, z)
        if problems:
            diffs.append({"index": i, "kind": "; ".join(problems), "baseline": b, "new": z})
        else:
            matched += 1
    res["compared"] = n
    res["matched"] = matched
    res["diff_count"] = len(diffs)
    res["diffs"] = diffs
    cn = max(len(base), len(coverage))
    cid = 0
    for i in range(min(len(base), len(coverage))):
        if base[i]["event_id"] == coverage[i]["event_id"]:
            cid += 1
    res["cov_compared"] = cn
    res["cov_eventid_agree"] = cid
    return res
def apply_exemptions(results, enabled):
    """ADR 0006 白名单命中判定（阶段B起在比对器内完成，acceptance-check 只读 report.json）。

    仅命中"键 =（通道, baseline.provider, baseline.event_id）"且 kind 以 D- 开头的条目
    （与旧 acceptance-check 对 report.diff 的 `D-实质分歧` 逐簇解析口径一致，实测 7 项/26 条）。
    enabled=True（默认）：条目移出 diffs 计入 exempted；enabled=False（G6 复核口径）：
    条目保留在 diffs 按 D 计入。两种口径下都返回白名单命中明细（whitelist_detail），
    供 report.json 的 whitelist_diff_hits 供 G6 核对"豁免未掩盖回归"。
    """
    exempted_detail = []
    whitelist_detail = []
    for r in results:
        kept = []
        ch_exempt = 0
        for d in r.get("diffs", []):
            b = d.get("baseline")
            key = (r["channel"], b["provider"], b["event_id"]) if (b and d["kind"].startswith("D-")) else None
            if key is not None and key in EXEMPTION_KEYS:
                rec = {"channel": r["channel"], "index": d["index"], "provider": b["provider"],
                       "event_id": b["event_id"], "root_cause": EXEMPTION_KEYS[key]}
                whitelist_detail.append(rec)
                if enabled:
                    exempted_detail.append(rec)
                    ch_exempt += 1
                    continue
            kept.append(d)
        r["diffs"] = kept
        r["diff_count"] = len(kept)
        r["exempted"] = ch_exempt
    return exempted_detail, whitelist_detail
def main():
    ap = argparse.ArgumentParser(description="基线解析 vs 新解析顺序对齐比对（主=基线同语言，副侧覆盖率）")
    ap.add_argument("--channels", nargs="*", default=CHANNELS)
    ap.add_argument("--no-exemptions", action="store_true",
                    help="关豁免复核口径（ADR 0006/G6）：白名单命中仍按 D 计入 diff")
    args = ap.parse_args()
    # G0：主比对语言随基线渲染语言自动探测（取第一个存在的基线文件；无文件则逐通道 BLOCKED）。
    primary_lang = "unknown"
    for c in args.channels:
        f = BASELINE_DIR / (c + ".evtx.txt")
        if f.exists():
            primary_lang = detect_baseline_lang(f)
            break
    results = [compare_channel(c, primary_lang) for c in args.channels]
    exemptions_on = not args.no_exemptions
    exempted_detail, whitelist_detail = apply_exemptions(results, exemptions_on)
    total_compared = sum(r.get("compared", 0) for r in results)
    total_matched = sum(r.get("matched", 0) for r in results)
    total_diff = sum(r.get("diff_count", 0) for r in results)
    blocked = [r for r in results if r["status"] != "ok"]
    passed = not blocked and total_diff == 0 and total_compared > 0
    lines = []
    a = lines.append
    a("# 对比报告（基线解析 vs 新解析）")
    a("")
    a("- 生成时间（本地）：" + datetime.now().strftime("%Y-%m-%d %H:%M:%S"))
    primary_short = "zh" if primary_lang == "zh-CN" else ("en" if primary_lang == "en-US" else "?")
    cov_short = "en" if primary_short == "zh" else "zh"
    a("- 主比对：基线（" + primary_lang + " 渲染，G0 自动探测） vs new-parsed/" + primary_short + "（同语言，严格五字段）")
    a("- 口径：EventID / Provider / Channel / TimeCreated（两边均为本地时间，直接比，容差2s）/ Message（空白归一）")
    a("- 对齐：同一文件出现顺序逐条对齐（基线Event[N]顺序对应新解析sequence_number顺序）")
    a("- 对照：new-parsed/" + cov_short + " 只计覆盖率（条数/EventID顺序一致率/空消息率），不逐条比Message")
    if exemptions_on:
        a("- 豁免（ADR 0006）：开启——白名单命中 " + str(len(exempted_detail)) + " 条已移出 diff 计入 exempted（--no-exemptions 关闭复核）")
    else:
        a("- 豁免（ADR 0006）：关闭（G6 复核口径）——白名单命中 " + str(len(whitelist_detail)) + " 条仍按 D 计入 diff")
    a("")
    a("## 结论")
    a("")
    if blocked:
        a("- 未完成：" + "; ".join(r["channel"] + ": " + r["status"] for r in blocked))
    elif total_compared == 0:
        a("- 未完成：无可比对事件")
    elif passed:
        a("- 通过：主链（基线vs en）共比对 " + str(total_compared) + " 条，全部一致")
    else:
        a("- 未通过：主链（基线vs en）共比对 " + str(total_compared) + " 条，一致 " + str(total_matched) + " 条，差异 " + str(total_diff) + " 条")
    a("")
    a("## 分通道统计（主：基线 vs " + primary_short + "）")
    a("")
    a("| 通道 | 基线数 | 新解析(" + primary_short + ") | 比对数 | 一致 | 差异 | " + primary_short + "空消息 | 状态 |")
    a("|---|---|---|---|---|---|---|---|")
    for r in results:
        a("| " + r["channel"] + " | " + str(r.get("baseline_count", "-")) + " | " + str(r.get("new_" + primary_short + "_count", "-")) + " | " + str(r.get("compared", "-")) + " | " + str(r.get("matched", "-")) + " | " + str(r.get("diff_count", "-")) + " | " + str(r.get(primary_short + "_empty_msg", "-")) + " | " + r["status"] + " |")
    a("")
    a("## 副侧覆盖率对照（基线 vs " + cov_short + "，不比Message）")
    a("")
    a("| 通道 | 新解析(" + cov_short + ") | 空消息 | EventID顺序一致/比对 |")
    a("|---|---|---|---|")
    for r in results:
        a("| " + r["channel"] + " | " + str(r.get("new_" + cov_short + "_count", "-")) + " | " + str(r.get(cov_short + "_empty_msg", "-")) + " | " + str(r.get("cov_eventid_agree", "-")) + "/" + str(r.get("cov_compared", "-")) + " |")
    a("")
    a("## 已知事项（比对中发现，非结论）")
    a("")
    a("- 新解析 CSV 内每 batch 夹带一行 BOM+表头重复（evtx-tools write_events 每批都写头），比对时已按 event_id 非数字过滤；建议上游修 storage/csv.rs 或 cmd_parse.rs。")
    a("- 基线 Date 看似UTC（带Z）实为本地时间：wevtutil /f:text 渲染本地时间但保留Z后缀（与raw XML SystemTime差+8h，Setup#0/EventRecordID=1实测）；evtx-tools convert_timestamp_local输出与raw UTC+8一致，无上游bug。基线小数秒被截断补零（.6720013→.6720000），容差覆盖。")
    a("")
    shown = 0
    for r in results:
        if shown >= TOP_N:
            break
        for d in r.get("diffs", []):
            if shown >= TOP_N:
                break
            shown += 1
            a("## 差异样例 " + str(shown) + "（" + r["channel"] + " #" + str(d["index"]) + "：" + d["kind"] + "）")
            a("")
            b, z = d["baseline"], d["new"]
            ns = "新解析" + ("zh" if r.get("primary_lang") == "zh-CN" else "en")
            if b is not None:
                a("- 基线 EventID=" + b["event_id"] + " Provider=" + b["provider"] + " Date=" + b["date_utc"])
                a("- 基线 Message（前300字）：" + b["message"][:300])
            else:
                a("- 基线：缺失（新解析多出）")
            if z is not None:
                a("- " + ns + " EventID=" + z["event_id"] + " Provider=" + z["provider"] + " Time=" + z["event_time_local"])
                a("- " + ns + " Message（前300字）：" + z["message"][:300])
            else:
                a("- " + ns + "：缺失（基线多出）")
            a("")
    if total_diff > shown:
        a("还有 " + str(total_diff - shown) + " 条差异未展示，见 report.diff。")
        a("")
    (TEST_ROOT / "report.md").write_text("\n".join(lines), encoding="utf-8")
    dl = []
    for r in results:
        new_label = "new-zh" if r.get("primary_lang") == "zh-CN" else "new-en"
        for d in r.get("diffs", []):
            b, z = d["baseline"], d["new"]
            dl.append("[" + r["channel"] + " #" + str(d["index"]) + "] " + d["kind"])
            dl.append("  baseline: EventID=" + (b["event_id"] if b else "-") + " Provider=" + (b["provider"] if b else "-"))
            dl.append("    Date=" + (b["date_utc"] if b else "-"))
            dl.append("    Msg=" + (b["message"][:500] if b else "-"))
            dl.append("  " + new_label + ":   EventID=" + (z["event_id"] if z else "-") + " Provider=" + (z["provider"] if z else "-"))
            dl.append("    Time=" + (z["event_time_local"] if z else "-"))
            dl.append("    Msg=" + (z["message"][:500] if z else "-"))
            dl.append("")
    (TEST_ROOT / "report.diff").write_text("\n".join(dl), encoding="utf-8")
    # 循环闭合输入口：机器可读汇总（下轮迭代对比用；exit code 保持人类结论语义不变）。
    import json as _json

    by_channel = {}
    for r in results:
        cats: dict = {}
        for d in r.get("diffs", []):
            head = d["kind"].split("；")[0].split(";")[0][:24]
            cats[head] = cats.get(head, 0) + 1
        by_channel[r["channel"]] = {
            "baseline": r.get("baseline_count"),
            "new_en": r.get("new_en_count"),
            "new_zh": r.get("new_zh_count"),
            "compared": r.get("compared"),
            "matched": r.get("matched"),
            "diff": r.get("diff_count"),
            "en_empty_msg": r.get("en_empty_msg"),
            "zh_empty_msg": r.get("zh_empty_msg"),
            "categories": cats,
            "exempted": r.get("exempted", 0),
            "status": r["status"],
        }
    summary = {
        "generated_local": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        "baseline_lang": primary_lang,
        "compared": total_compared,
        "matched": total_matched,
        "diff": total_diff,
        # 阶段B（ADR 0006）：豁免统计。exempted = 默认口径下移出 diff 的白名单命中；
        # whitelist_diff_hits = 两种口径下白名单键在 diff 中的命中（G6 关豁免复核用）。
        "exemptions_enabled": exemptions_on,
        "exempted": len(exempted_detail) if exemptions_on else 0,
        "exempted_detail": exempted_detail if exemptions_on else [],
        "whitelist_diff_hits": len(whitelist_detail),
        "whitelist_diff_detail": whitelist_detail,
        "verdict": ("BLOCKED" if blocked else ("PASS" if passed else "DIFF")),
        "by_channel": by_channel,
    }
    (TEST_ROOT / "report.json").write_text(_json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print("compared=" + str(total_compared) + " matched=" + str(total_matched) + " diff=" + str(total_diff)
          + " exempted=" + str(len(exempted_detail) if exemptions_on else 0))
    if passed:
        print("PASS")
        return 0
    if blocked:
        print("BLOCKED")
        return 2
    print("DONE-WITH-DIFF")
    return 1
if __name__ == "__main__":
    sys.exit(main())
