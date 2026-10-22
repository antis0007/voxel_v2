"""Summarize the nested resident-upload wall timer in real native route CSVs."""
import argparse
import csv
import json
import math
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument("case", type=Path)
args = p.parse_args()
rows = []
all_frames = failed = 0
with (args.case / "route-frames.csv").open(encoding="utf-8-sig", newline="") as f:
    reader = csv.DictReader(f)
    names = reader.fieldnames
    assert len(names) == len(set(names)) == 25
    assert names[-1] == "resident_upload_wall_ms"
    for row in reader:
        assert None not in row and None not in row.values(), "Incomplete CSV record"
        all_frames += 1
        failed += row["render_ok"] != "true"
        stream = float(row["stream_ms"])
        upload = float(row["resident_upload_wall_ms"])
        assert math.isfinite(upload) and 0 <= upload <= stream + 0.001
        if row["measuring_at_render"] != "true" or row["horizon_ready"] != "true":
            continue
        item = {k: float(row[k]) for k in (
            "seconds", "total_ms", "stream_ms", "resident_upload_wall_ms",
            "horizon_ms", "gameplay_ms", "snapshot_ms", "render_cpu_ms")}
        item["frame"] = int(row["frame"])
        rows.append(item)
assert rows, "No measured ready render attempts"


def stats(key):
    values = sorted(row[key] for row in rows)
    return {"mean": sum(values) / len(values),
            "p95": values[math.ceil(len(values) * 0.95) - 1],
            "max": values[-1], "over_50ms": sum(v > 50 for v in values)}


report = {
    "scope": "Actual native measured-ready attempts. Resident upload wall time is nested in stream_ms; it includes preparation/admission/checks inside upload_finished, not only GPU writes. It does not identify an allocator, driver, validation or other underlying operation.",
    "csv_columns": names, "csv_and_nested_timer_valid": True,
    "all_render_attempts": all_frames, "failed_renders": failed,
    "measured_ready_attempts": len(rows),
    "stream_ms": stats("stream_ms"),
    "resident_upload_wall_ms": stats("resident_upload_wall_ms"),
    "total_ms": stats("total_ms"),
    "top_stream_frames": sorted(rows, key=lambda r: r["stream_ms"], reverse=True)[:10],
    "top_upload_frames": sorted(rows, key=lambda r: r["resident_upload_wall_ms"], reverse=True)[:10],
}
output = args.case / "resident-upload-review.json"
output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
print(json.dumps({k: v for k, v in report.items() if not k.startswith("top_")}, indent=2))
