#!/usr/bin/env python3
"""Generate a static weekly npm downloads SVG for the README."""

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


def parse_args():
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", default="@leonw24/open-codex")
    parser.add_argument("--weeks", type=int, default=12)
    parser.add_argument("--output", default=".github/npm-weekly-downloads.svg")
    return parser.parse_args()


def latest_complete_period_end():
    return date.today() - timedelta(days=1)


def fetch_daily_downloads(package_name, weeks):
    end = latest_complete_period_end()
    start = end - timedelta(days=(weeks * 7) - 1)
    encoded = urllib.parse.quote(package_name, safe="")
    url = "https://api.npmjs.org/downloads/range/{start}:{end}/{package}".format(
        start=start.isoformat(),
        end=end.isoformat(),
        package=encoded,
    )
    try:
        with urllib.request.urlopen(url, timeout=20) as response:
            payload = json.loads(response.read().decode("utf-8"))
    except Exception:
        output = subprocess.check_output(["curl", "-fsSL", url])
        payload = json.loads(output.decode("utf-8"))
    return payload.get("downloads", [])


def aggregate_weeks(daily_rows, weeks):
    end = latest_complete_period_end()
    first_week = end - timedelta(days=(weeks * 7) - 1)
    buckets = []
    totals = {}
    for i in range(weeks):
        start = first_week + timedelta(days=i * 7)
        buckets.append(start)
        totals[start] = 0

    for row in daily_rows:
        day = datetime.strptime(row["day"], "%Y-%m-%d").date()
        index = (day - first_week).days // 7
        if 0 <= index < weeks:
            totals[buckets[index]] += int(row.get("downloads", 0))

    return [(start, totals[start]) for start in buckets]


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


def generate_svg(package_name, weekly_rows):
    width = 960
    height = 300
    left = 70
    right = 32
    top = 58
    bottom = 56
    chart_w = width - left - right
    chart_h = height - top - bottom
    max_downloads = max([downloads for _, downloads in weekly_rows] or [0])
    upper = nice_upper_bound(max_downloads)

    def x_for(index):
        if len(weekly_rows) == 1:
            return left + chart_w / 2.0
        return left + (chart_w * index / float(len(weekly_rows) - 1))

    def y_for(value):
        return top + chart_h - (chart_h * value / float(upper))

    points = []
    for index, (_, downloads) in enumerate(weekly_rows):
        points.append((x_for(index), y_for(downloads), downloads))

    if points:
        path = "M " + " L ".join("{:.1f} {:.1f}".format(x, y) for x, y, _ in points)
    else:
        path = ""

    grid_lines = []
    for step in range(5):
        value = int(upper * step / 4)
        y = y_for(value)
        grid_lines.append(
            '<line x1="{left}" y1="{y:.1f}" x2="{right_x}" y2="{y:.1f}" stroke="#243047" stroke-width="1" />'.format(
                left=left,
                y=y,
                right_x=width - right,
            )
        )
        grid_lines.append(
            svg_text(
                18,
                y + 4,
                fmt_count(value),
                'font-size="12" fill="#8b9ab8"',
            )
        )

    labels = []
    if weekly_rows:
        label_indexes = sorted(set([0, len(weekly_rows) // 2, len(weekly_rows) - 1]))
        for index in label_indexes:
            start, _ = weekly_rows[index]
            labels.append(
                svg_text(
                    x_for(index),
                    height - 22,
                    "{} {}".format(start.strftime("%b"), start.day),
                    'font-size="12" fill="#8b9ab8" text-anchor="middle"',
                )
            )

    dots = []
    for x, y, downloads in points:
        dots.append(
            '<circle cx="{x:.1f}" cy="{y:.1f}" r="4" fill="#8b5cf6" stroke="#dbe7ff" stroke-width="2"><title>{downloads} downloads</title></circle>'.format(
                x=x,
                y=y,
                downloads=downloads,
            )
        )

    latest = weekly_rows[-1][1] if weekly_rows else 0
    total_window = sum(downloads for _, downloads in weekly_rows)
    through = (weekly_rows[-1][0] + timedelta(days=6)).isoformat() if weekly_rows else ""

    return """<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" role="img" aria-labelledby="title desc">
  <title id="title">{package_name} weekly npm downloads</title>
  <desc id="desc">Weekly npm downloads over the last {weeks} complete 7-day periods. Latest period: {latest} downloads.</desc>
  <defs>
    <linearGradient id="line" x1="0" x2="1" y1="0" y2="0">
      <stop offset="0%" stop-color="#22d3ee" />
      <stop offset="55%" stop-color="#8b5cf6" />
      <stop offset="100%" stop-color="#f472b6" />
    </linearGradient>
    <linearGradient id="fill" x1="0" x2="0" y1="0" y2="1">
      <stop offset="0%" stop-color="#8b5cf6" stop-opacity="0.22" />
      <stop offset="100%" stop-color="#8b5cf6" stop-opacity="0" />
    </linearGradient>
  </defs>
  <rect width="{width}" height="{height}" rx="16" fill="#0b1020" />
  <rect x="1" y="1" width="{inner_w}" height="{inner_h}" rx="15" fill="none" stroke="#26324a" />
  <text x="{left}" y="30" font-family="Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#f8fafc">npm weekly downloads</text>
  <text x="{left}" y="50" font-family="Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="12" fill="#8b9ab8">{package_name} · latest 7-day period {latest_fmt} · {total_fmt} over {weeks} periods · through {through}</text>
  <g font-family="Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif">
    {grid}
    <line x1="{left}" y1="{baseline:.1f}" x2="{right_x}" y2="{baseline:.1f}" stroke="#3a4867" stroke-width="1.2" />
    <path d="{area_path} L {last_x:.1f} {baseline:.1f} L {left:.1f} {baseline:.1f} Z" fill="url(#fill)" />
    <path d="{path}" fill="none" stroke="url(#line)" stroke-width="4" stroke-linecap="round" stroke-linejoin="round" />
    {dots}
    {labels}
  </g>
</svg>
""".format(
        width=width,
        height=height,
        inner_w=width - 2,
        inner_h=height - 2,
        package_name=escape(package_name),
        weeks=len(weekly_rows),
        latest=latest,
        latest_fmt=fmt_count(latest),
        total_fmt=fmt_count(total_window),
        through=through,
        left=left,
        baseline=top + chart_h,
        right_x=width - right,
        grid="\n    ".join(grid_lines),
        path=path,
        area_path=path,
        last_x=points[-1][0] if points else left,
        dots="\n    ".join(dots),
        labels="\n    ".join(labels),
    )


def main():
    args = parse_args()
    daily_rows = fetch_daily_downloads(args.package, args.weeks)
    weekly_rows = aggregate_weeks(daily_rows, args.weeks)
    svg = generate_svg(args.package, weekly_rows)
    with open(args.output, "w") as handle:
        handle.write(svg)
    return 0


if __name__ == "__main__":
    sys.exit(main())
