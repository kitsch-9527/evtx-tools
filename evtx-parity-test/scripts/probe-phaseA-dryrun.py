"""阶段A dry-run 探针：验证两条归一规则在当前数据上的影响面（只读，不改 06）。"""
import importlib.util
from pathlib import Path

spec = importlib.util.spec_from_file_location("c06", Path(__file__).parent / "06-compare.py")
c06 = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c06)

BS = chr(92)  # backslash
Q = chr(63)   # question mark
WER_OLD = BS + BS + Q + BS          # \\?\
WER_NEW = BS + BS + BS              # \\\
LRM = chr(0x200E)
RLM = chr(0x200F)


def canon_wer_only(s):
    return c06.WS_RE.sub(" ", s.replace(WER_OLD, WER_NEW).replace(";", " ")).strip()


def canon_lrm_only(s):
    return c06.WS_RE.sub(" ", s.replace(LRM, "").replace(RLM, "").replace(";", " ")).strip()


def canon_both(s):
    return c06.WS_RE.sub(" ", s.replace(WER_OLD, WER_NEW).replace(LRM, "").replace(RLM, "").replace(";", " ")).strip()


def survey(ch):
    base = c06.parse_baseline(c06.BASELINE_DIR / (ch + ".evtx.txt"))
    en = c06.load_new_csv(c06.NEW_EN_DIR / (ch + ".csv"))
    out = []
    for i in range(max(len(base), len(en))):
        b = base[i] if i < len(base) else None
        z = en[i] if i < len(en) else None
        if b is None or z is None:
            continue
        probs = c06.cmp_pair(b, z)
        if not probs:
            continue
        bm = c06.norm_msg_time_frac(b["message"])
        zm = c06.norm_msg_time_frac(z["message"])
        if "cannot be found" in zm or (";;" in zm and len(zm) < 200):
            continue  # B/C 不受 canon 影响
        out.append((i, b, z, bm, zm))
    return out


total_cleared = {"wer": 0, "lrm": 0, "both": 0}
all_rows = []
for ch in c06.CHANNELS:
    rows = survey(ch)
    all_rows.extend((ch,) + r for r in rows)
    for ch2, i, b, z, bm, zm in [(x[0], x[1], x[2], x[3], x[4], x[5]) for x in all_rows if x[0] == ch]:
        pass
print("D/消息类 diff 总数（排除B/C与缺失）:", len(all_rows))
for ch, i, b, z, bm, zm in all_rows:
    w = canon_wer_only(bm) == canon_wer_only(zm)
    l = canon_lrm_only(bm) == canon_lrm_only(zm)
    bo = canon_both(bm) == canon_both(zm)
    if bo:
        tag = []
        if w and not l: tag = ["WER?"]
        elif l and not w: tag = ["LRM"]
        elif w and l: tag = ["WER?+LRM"]
        total_cleared["both"] += 1
        if w: total_cleared["wer"] += 1
        if l: total_cleared["lrm"] += 1
        print("cleared", ch, "#"+str(i), "EID="+z["event_id"], z["provider"], tag)
    else:
        # 检查该 diff 里是否含 \\?\ 或 LRM 相关形态（诊断用）
        has_wer_shape = (WER_OLD in bm or WER_NEW in bm or WER_OLD in zm or WER_NEW in zm)
        has_lrm_shape = (LRM in bm or LRM in zm or (Q + chr(0x4E00)) in bm)
        print("kept   ", ch, "#"+str(i), "EID="+z["event_id"], z["provider"],
              "wer_shape=" + str(has_wer_shape), "lrm_shape=" + str(has_lrm_shape))
print("summary:", total_cleared)
