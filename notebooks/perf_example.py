"""Presentation for the synthetic perf.data sample-accounting example."""

import hashlib
import json
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
from matplotlib.patches import Patch, Rectangle
from matplotlib.ticker import MaxNLocator
from vgi_notebooks import finish

SOURCE = "Source: synthetic callchain.data golden fixture · VGI perf v0.4.0 · raw addresses, no symbolication"
COLORS = {"/usr/bin/loop": "#287d85", "/usr/lib/libc.so.6": "#bd7735", "[unmapped]": "#888888"}


def validate_depth(value):
    """Validate the visible analytical depth cutoff."""
    if type(value) is not int or not 1 <= value <= 8:
        raise ValueError("MAX_FRAMES must be an integer from 1 to 8.")


def verify_fixture():
    """Verify the unchanged golden fixture before worker invocation."""
    manifest = json.loads(Path("fixtures/provenance.json").read_text())
    digest = hashlib.sha256(Path("fixtures/callchain.data").read_bytes()).hexdigest()
    if digest != manifest["sha256"]:
        raise ValueError("Fixture hash differs from its recorded provenance.")
    return digest


def stacks(frame_data, self_data, max_frames):
    """Retain omitted and absent frames visibly beside the selected frames."""
    fig, ax = plt.subplots(figsize=(11, 6.3))
    ax.grid(False)
    max_depth = int(frame_data.frame_position.max())
    for row in frame_data.itertuples():
        x = row.frame_position - 1
        y = row.sample_id - 1
        color = COLORS.get(row.object, "#888888") if row.retained else "#e5e5e5"
        ax.add_patch(
            Rectangle(
                (x - 0.48, y - 0.43),
                0.96,
                0.86,
                facecolor=color,
                edgecolor="white",
                hatch=None if row.retained else "//",
            )
        )
        text = f"{Path(row.object).name}\n0x{row.ip:x}" + ("\nOmitted" if not row.retained else "")
        ax.text(
            x, y, text, ha="center", va="center", fontsize=10, color="white" if row.retained else "#333333"
        )
    for row in self_data.itertuples():
        for pos in range(1, max_depth + 1):
            if not ((frame_data.sample_id == row.sample_id) & (frame_data.frame_position == pos)).any():
                ax.text(
                    pos - 1,
                    row.sample_id - 1,
                    "No recorded frame",
                    ha="center",
                    va="center",
                    fontsize=10,
                    color="#666666",
                )
    ax.set(
        xlim=(-0.5, max_depth - 0.5),
        ylim=(len(self_data) - 0.5, -0.5),
        xticks=range(max_depth),
        xticklabels=[f"Frame {i + 1}" for i in range(max_depth)],
        yticks=range(len(self_data)),
        yticklabels=[
            f"Sample {r.sample_id} · {r.comm}\nPID {r.pid} · CPU {r.cpu}" for r in self_data.itertuples()
        ],
        xlabel="Original callchain position · first frame matches sampled IP in this fixture",
    )
    fig.legend(
        handles=[Patch(facecolor=COLORS[k], label=Path(k).name) for k in COLORS if k != "[unmapped]"]
        + [Patch(facecolor="#e5e5e5", hatch="//", label="Omitted by cutoff")],
        loc="lower center",
        bbox_to_anchor=(0.56, 0.76),
        ncol=3,
        frameon=False,
    )
    finish(
        fig,
        "A depth cutoff removes caller context, not samples",
        f"Retain at most {max_frames} frames per sample · grey cells retain the omitted evidence",
        SOURCE,
        name="01-retained-stacks",
    )


def counts(comparison, max_frames):
    """Compare exclusive samples, inclusive pairs and repeated frame occurrences."""
    x = np.arange(len(comparison))
    fig, ax = plt.subplots(figsize=(11, 6.0))
    for j, (column, label, color) in enumerate(
        [
            ("self_samples", "Self samples", "#287d85"),
            ("inclusive_samples", "Inclusive distinct samples", "#bd7735"),
            ("frame_occurrences", "Frame occurrences", "#7462a0"),
        ]
    ):
        bars = ax.bar(x + (j - 1) * 0.25, comparison[column], width=0.23, label=label, color=color)
        ax.bar_label(bars, fmt="%d", padding=4)
    ax.set(
        xticks=x,
        xticklabels=[Path(v).name for v in comparison.object],
        ylabel="Count · definition varies by series",
        ylim=(0, 4.8),
    )
    ax.yaxis.set_major_locator(MaxNLocator(integer=True))
    fig.legend(
        *ax.get_legend_handles_labels(),
        loc="lower center",
        bbox_to_anchor=(0.56, 0.76),
        ncol=3,
        frameon=False,
    )
    finish(
        fig,
        "Inclusive samples overlap across mapped objects",
        f"Retain {max_frames} frames · self counts partition samples; inclusive counts do not",
        SOURCE,
        name="02-counting-units",
    )
