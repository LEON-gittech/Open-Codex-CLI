#!/usr/bin/env python3
"""Generate a static npm cumulative downloads SVG for the README.

The chart plots running-total downloads weekly so the trend over the
package's lifetime is visible at a glance. The line is by definition
monotonically non-decreasing.
"""

import argparse
import json
import math
import subprocess
import sys
import urllib.parse
import urllib.request
from datetime import date
from datetime import datetime
from datetime import timedelta
from xml.sax.saxutils import escape


# npm range API allows up to ~365 days per call; chunk further back if needed.
_NPM_RANGE_MAX_DAYS = 365


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", default="@leonw24/open-codex")
    parser.add_argument(
        "--weeks",
        type=int,
        default=26,
        help="Number of 7-day buckets to plot as the trend window.",
    )
    parser.add_argument("--output", default=".github/npm-total-downloads.svg")
    return parser.parse_args()


def latest_complete_period_end():
    return date.today() - timedelta(days=1)


def _fetch_range(package_encoded, start, end):
    url = "https://api.npmjs.org/downloads/range/{start}:{end}/{package}".format(
        start=start.isoformat(),
        end=end.isoformat(),
        package=package_encoded,
    )
    try:
        with urllib.request.urlopen(url, timeout=20) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except Exception:
        output = subprocess.check_output(["curl", "-fsSL", url])
        payload = json.loads(output.decode("utf-8"))
    return payload.get("downloads", [])


def _package_first_release(package_name):
    """Best-effort lookup of the package's first publish date.

    Falls back to None on any failure; callers should treat None as
    "history begins today" so the chart still renders.
    """
    encoded = urllib.parse.quote(package_name, safe="")
    url = "https://registry.npmjs.org/{}".format(encoded)
    try:
        with urllib.request.urlopen(url, timeout=20) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except Exception:
        try:
            output = subprocess.check_output(["curl", "-fsSL", url])
            payload = json.loads(output.decode("utf-8"))
        except Exception:
            return None
    times = payload.get("time", {})
    created = times.get("created") or times.get("modified")
    if not created:
        return None
    try:
        return datetime.fromisoformat(created.replace("Z", "+00:00")).date()
    except ValueError:
        return None


def fetch_daily_downloads_since(package_name, since):
    """Fetch daily download counts in [since, latest_complete_period_end]."""
    encoded = urllib.parse.quote(package_name, safe="")
    end = latest_complete_period_end()
    rows = []
    chunk_start = since
    while chunk_start <= end:
        chunk_end = min(end, chunk_start + timedelta(days=_NPM_RANGE_MAX_DAYS - 1))
        rows.extend(_fetch_range(encoded, chunk_start, chunk_end))
        chunk_start = chunk_end + timedelta(days=1)
    return rows


def aggregate_cumulative(daily_rows, window_weeks):
    """Bucket daily rows into 7-day periods of the visible window and emit
    the running total at each bucket boundary.

    Walks every bucket back to the earliest known day (no artificial floor at
    `window_weeks`), then drops any leading buckets whose cumulative total is
    still zero. The visual line therefore starts at the first week the
    package saw any downloads — no flat run of zeros before launch — and
    grows monotonically from there.

    When real history is longer than `window_weeks`, the displayed range is
    clamped to the most recent `window_weeks` periods so the dashboard stays
    readable; the displayed line still starts at the lifetime baseline
    because the running total carries pre-window downloads.
    """
    end = latest_complete_period_end()

    by_day = {}
    for row in daily_rows:
        try:
            day = datetime.strptime(row["day"], "%Y-%m-%d").date()
        except (KeyError, ValueError):
            continue
        by_day[day] = int(row.get("downloads", 0))

    if not by_day:
        return []

    earliest_day = min(by_day)
    # Align to a Monday-anchored 7-day bucket whose end day matches `end` so
    # the rightmost label is always the latest complete period.
    total_days = (end - earliest_day).days + 1
    bucket_count = max(1, math.ceil(total_days / 7.0))
    first_bucket_start = end - timedelta(days=bucket_count * 7 - 1)

    buckets = []
    running = 0
    for i in range(bucket_count):
        start = first_bucket_start + timedelta(days=i * 7)
        bucket_end = start + timedelta(days=6)
        bucket_sum = sum(
            count
            for day, count in by_day.items()
            if start <= day <= bucket_end
        )
        running += bucket_sum
        buckets.append((start, running))

    # Drop leading buckets that are still at zero (pre-launch padding).
    first_nonzero = next(
        (idx for idx, (_, total) in enumerate(buckets) if total > 0),
        len(buckets),
    )
    visible = buckets[first_nonzero:]

    # If real history exceeds the trend window, keep only the most recent
    # `window_weeks` buckets to keep the chart readable.
    if len(visible) > window_weeks:
        visible = visible[-window_weeks:]
    return visible


def nice_upper_bound(value):
    if value <= 0:
        return 10
    magnitude = 10 ** int(math.floor(math.log10(value)))
    normalized = float(value) / magnitude
    if normalized <= 2:
        nice = 2
    elif normalized <= 5:
        nice = 5
    else:
        nice = 10
    return nice * magnitude


def fmt_count(value):
    if value >= 1000000:
        return "{:.1f}M".format(value / 1000000.0).rstrip("0").rstrip(".")
    if value >= 1000:
        return "{:.1f}k".format(value / 1000.0).rstrip("0").rstrip(".")
    return str(value)


def svg_text(x, y, text, extra=""):
    return '<text x="{x}" y="{y}" {extra}>{text}</text>'.format(
        x=x,
        y=y,
        extra=extra,
        text=escape(text),
    )


def generate_svg(package_name, cumulative_rows):
    # Portrait "stat card" sized to sit beside the unleashed banner in the
    # README 8:2 row. The banner is ~16:9 at 760 wide → ~427 tall on render;
    # this card's SVG is 400×900 so at the README's 190 display width it
    # comes out ~427 tall and lines up with the banner edge.
    width = 400
    height = 900
    pad = 32
    chart_top = 360
    chart_bottom = 760
    chart_w = width - 2 * pad
    chart_h = chart_bottom - chart_top

    max_downloads = max([downloads for _, downloads in cumulative_rows] or [0])
    # Cap the y-axis at "current cumulative + 1k" so the latest point sits
    # near the top of the trend area instead of getting squashed when
    # `nice_upper_bound` jumps to the next 2× / 5× / 10× tier.
    upper = max(max_downloads + 1000, 1000)

    def x_for(index):
        if len(cumulative_rows) <= 1:
            return pad + chart_w / 2.0
        return pad + (chart_w * index / float(len(cumulative_rows) - 1))

    def y_for(value):
        if upper <= 0:
            return chart_bottom
        return chart_bottom - (chart_h * value / float(upper))

    points = []
    for index, (_, downloads) in enumerate(cumulative_rows):
        points.append((x_for(index), y_for(downloads), downloads))

    if points:
        path = "M " + " L ".join("{:.1f} {:.1f}".format(x, y) for x, y, _ in points)
    else:
        path = ""

    # Horizontal grid lines spanning the card width.
    grid_lines = []
    for step in range(5):
        value = int(upper * step / 4)
        y = y_for(value)
        grid_lines.append(
            '<line x1="{left}" y1="{y:.1f}" x2="{right}" y2="{y:.1f}" stroke="#243047" stroke-width="1" />'.format(
                left=pad,
                y=y,
                right=width - pad,
            )
        )

    # Date labels at start / middle / end of the trend.
    labels = []
    if cumulative_rows:
        label_indexes = sorted(
            set([0, len(cumulative_rows) // 2, len(cumulative_rows) - 1])
        )
        for index in label_indexes:
            start, _ = cumulative_rows[index]
            anchor = "middle"
            if index == 0:
                anchor = "start"
            elif index == len(cumulative_rows) - 1:
                anchor = "end"
            labels.append(
                svg_text(
                    x_for(index),
                    chart_bottom + 36,
                    "{} {}".format(start.strftime("%b"), start.day),
                    'font-size="24" fill="#8b9ab8" text-anchor="{}"'.format(anchor),
                )
            )

    dots = []
    for x, y, downloads in points:
        dots.append(
            '<circle cx="{x:.1f}" cy="{y:.1f}" r="8" fill="#8b5cf6" stroke="#dbe7ff" stroke-width="3"><title>{downloads} downloads</title></circle>'.format(
                x=x,
                y=y,
                downloads=downloads,
            )
        )

    latest = cumulative_rows[-1][1] if cumulative_rows else 0
    first = cumulative_rows[0][1] if cumulative_rows else 0
    window_delta = latest - first
    through = (
        (cumulative_rows[-1][0] + timedelta(days=6)).isoformat()
        if cumulative_rows
        else ""
    )

    return """<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
  <title id="title">{package_name} cumulative npm downloads</title>
  <desc id="desc">Cumulative npm downloads sampled at weekly boundaries across {weeks} complete 7-day periods. Latest cumulative total: {latest} downloads.</desc>
  <defs>
    <linearGradient id="line" x1="0" x2="0" y1="1" y2="0">
      <stop offset="0%" stop-color="#22d3ee" />
      <stop offset="55%" stop-color="#8b5cf6" />
      <stop offset="100%" stop-color="#f472b6" />
    </linearGradient>
    <linearGradient id="fill" x1="0" x2="0" y1="0" y2="1">
      <stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.22" />
      <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0" />
    </linearGradient>
  </defs>
  <rect width="{width}" height="{height}" rx="22" fill="#0b1020" />
  <rect x="1" y="1" width="{inner_w}" height="{inner_h}" rx="21" fill="none" stroke="#26324a" />
  <g font-family="Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif">
    <text x="{cx}" y="80" font-size="32" font-weight="700" fill="#f8fafc" text-anchor="middle">npm downloads</text>
    <text x="{cx}" y="118" font-size="22" fill="#8b9ab8" text-anchor="middle">cumulative · {weeks} weeks</text>
    <text x="{cx}" y="240" font-size="96" font-weight="800" fill="#f8fafc" text-anchor="middle">{latest_fmt}</text>
    <text x="{cx}" y="290" font-size="24" fill="#22d3ee" text-anchor="middle">+{delta_fmt} since {first_fmt}</text>
    {grid}
    <line x1="{pad}" y1="{baseline:.1f}" x2="{right}" y2="{baseline:.1f}" stroke="#3a4867" stroke-width="1.4" />
    <path d="{area_path} L {last_x:.1f} {baseline:.1f} L {first_x:.1f} {baseline:.1f} Z" fill="url(#fill)" />
    <path d="{path}" fill="none" stroke="url(#line)" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" />
    {dots}
    {labels}
    <text x="{cx}" y="{footer_y}" font-size="22" fill="#5d6b88" text-anchor="middle">through {through}</text>
  </g>
</svg>
""".format(
        width=width,
        height=height,
        inner_w=width - 2,
        inner_h=height - 2,
        package_name=escape(package_name),
        weeks=len(cumulative_rows),
        latest=latest,
        latest_fmt=fmt_count(latest),
        delta_fmt=fmt_count(window_delta),
        first_fmt=fmt_count(first),
        through=through,
        pad=pad,
        cx=width / 2.0,
        baseline=chart_bottom,
        right=width - pad,
        footer_y=height - 32,
        grid="\n    ".join(grid_lines),
        path=path,
        area_path=path,
        first_x=points[0][0] if points else pad,
        last_x=points[-1][0] if points else pad,
        dots="\n    ".join(dots),
        labels="\n    ".join(labels),
    )


def main():
    args = parse_args()
    end = latest_complete_period_end()
    visible_start = end - timedelta(days=(args.weeks * 7) - 1)
    first_release = _package_first_release(args.package)
    history_start = first_release if first_release else visible_start
    if history_start > visible_start:
        history_start = visible_start
    daily_rows = fetch_daily_downloads_since(args.package, history_start)
    cumulative_rows = aggregate_cumulative(daily_rows, args.weeks)
    svg = generate_svg(args.package, cumulative_rows)
    with open(args.output, "w") as handle:
        handle.write(svg)
    return 0


if __name__ == "__main__":
    sys.exit(main())
