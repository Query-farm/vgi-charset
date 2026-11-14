# Character encoding notebooks

[01 · Clean decoding is not the same as correct text](01_clean_bytes_wrong_words.ipynb)
follows four synthetic byte samples through detection, explicit decoding, a
mojibake repair comparison and an export round trip. The two saved charts compare
text preservation and byte counts. The final experiment changes the export codec
from Windows-1252 to UTF-8 while leaving every input byte unchanged.

## Run the notebook

Install [uv](https://docs.astral.sh/uv/), then use this directory:

```sh
cd notebooks
uv sync --locked
uv run --locked python execute.py
```

In VS Code or another notebook editor, select this directory's `.venv/bin/python`
as the kernel and run all cells. The notebook may start in `notebooks/` or its
repository root. The entire notebook directory also runs when copied elsewhere;
it needs no worker checkout, Rust compiler or root virtual environment.

The visible SQL connection uses the published native worker directly:

```sql
INSTALL vgi FROM community;
LOAD vgi;
ATTACH 'charset' (TYPE vgi,
  LOCATION 'github-auto://Query-farm/vgi-charset@v0.4.0');
```

VGI chooses the platform archive, verifies its published SHA256 sidecar and
caches it. This release supplies macOS arm64, Linux amd64 and Linux arm64 builds;
another platform needs an available worker release. The notebook environment
pins Haybarn 2.0.0rc11 and locks the Python visualization dependencies. Initial
dependency/extension/worker downloads need network access; the analysis is local
and uses no credentials or external data service. To adopt a different worker
release, change the visible tag and rerun all cells and comparisons.

Parameters and connection/analytical SQL remain expanded. Setup, plotting and
prose-generation code are folded, with meaningful tables and charts visible.

## Experiment and execution guarantees

From the notebook directory:

```sh
uv run --locked python execute.py --parameters '{"EXPORT_ENCODING":"UTF-8"}' \
  --output-dir validation/utf8
```

`EXPORT_ENCODING` accepts `windows-1252`, `Shift_JIS` or `UTF-8`. Restart and run
all after changing it; reset to `windows-1252`. The matrix covers all three
encodings, while the outlined column, export table and paired byte bars follow
the selected parameter. Check exact round-trip text before comparing storage.

The runner uses a fresh kernel and saves only after successful execution. It
requires a separate destination for parameter overrides and rejects overwriting
the source notebook. Experiment copies carry their helper, assets, environment
and guide. Generated logs, extracted charts and copies are ignored under
`validation/`; the main notebook includes embedded outputs.

## Source and interpretation

All inputs are fixed, synthetic examples written in visible SQL: Windows-1252
accent bytes, a short Shift_JIS sample, a UTF-16LE BOM greeting and a UTF-8 emoji
greeting. The expected text and source labels are independently specified.
Character operations run through the real worker; Python does not substitute
its own codecs. Original bytes remain available alongside both decoding paths.

`detect_confidence` is a decode-cleanliness proxy, not a calibrated probability.
The ambiguous byte example deliberately has no asserted intended text. The
mojibake controls distinguish a known corruption from intentionally literal
characters; even a proposed repair can alter meaning. Neither these examples nor
their counts estimate detector or repair quality on a broader population.

Export preservation means exact Unicode-text equality after decoding the exported
bytes with the same encoding label. Legacy encoders can emit HTML numeric
references for unrepresentable characters. No HTML unescaping is done. Byte
counts exclude transport/file metadata and are different from character counts.

Validation found a release-specific limitation: **v0.4.0 writes UTF-8 bytes when
asked to export UTF-16LE/BE**. Current repository source has a fix, but the pinned
published artifact does not. This walkthrough therefore restricts export choices
to the three verified codecs above. UTF-16 decoding, including the BOM fixture,
works separately. The latest source's full SQL suite and the released artifact
must not be described as interchangeable. No worker implementation or root
dependency lock was changed for these examples.

The official Strata Sun and VGI assets are unchanged copies of the Query.Farm
media kit used in the approved notebook standard. Strata Sun closes the notebook
as well as opening it.

References: [worker documentation](https://github.com/Query-farm/vgi-charset),
[release v0.4.0](https://github.com/Query-farm/vgi-charset/releases/tag/v0.4.0).
