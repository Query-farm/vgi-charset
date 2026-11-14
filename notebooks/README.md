# Character encoding examples

[01 · Clean decoding is not the same as correct text](01_clean_bytes_wrong_words.ipynb)
follows four synthetic byte samples through detection, explicit decoding, a mojibake
repair comparison and export round trips. It retains the original investigation and
two figures, with a supported-codec preflight and separate empty, missing and invalid
UTF-8 controls. All eight public SQL surfaces contribute to the investigation.

Use **Open in Colab**, run the install cell, then **Run all**. No clone, data file,
account or companion download is required. The base shared package preserves compatible
preloaded plotting/data libraries. A fresh Python 3.12 notebook-only execution with
preloaded libraries is tested; an actual hosted Colab session has not been tested.

For local execution, install Python 3.11+ and [uv](https://docs.astral.sh/uv/):

```sh
cd notebooks
uv sync --locked --extra runner
uv run --locked --extra runner vgi-notebooks
```

The optional `runner` extra supplies a local execution kernel. For interactive Jupyter,
register it with `uv run --locked --extra runner python -m ipykernel install --user
--name vgi-charset-notebooks --display-name 'VGI Charset notebooks'`, then select it.
Restart and run all after changing parameters.

The visible SQL connection uses the published native worker:

```sql
INSTALL vgi FROM community;
LOAD vgi;
ATTACH 'charset' (TYPE vgi,
  LOCATION 'github-auto://Query-farm/vgi-charset@v0.4.0');
```

Haybarn 2.0.0rc11 downloads and verifies the platform's release archive and SHA256
sidecar. No Rust compiler, checkout binary or dependency override is needed. Initial
Python, extension and worker downloads need network access; the analysis is local.
Native validation ran on macOS Apple silicon; platform availability depends on release
assets. To adopt another worker release, change the visible tag and rerun all checks.
The shared connection opens an in-memory database with an empty temporary SQL-secret
directory, disables persistent secrets, and closes explicitly at the end.

## One-parameter experiment

```sh
uv run --locked --extra runner vgi-notebooks \
  --parameters '{"EXPORT_ENCODING":"UTF-8"}' --output-dir validation/utf8
```

The parameter accepts `windows-1252`, `Shift_JIS` or `UTF-8`. The default Windows-1252
export preserves 2 of 4 strings exactly and totals 42 bytes; UTF-8 preserves 4 of 4
and totals 23 bytes. UTF-8 does not shrink every payload: the accent sample grows
from 4 to 5 bytes, while the Japanese numeric-reference representation shrinks from
24 to 9. The complete 12-row comparison, source bytes, detector guesses and repair
controls stay unchanged. The selected table, matrix outline and paired byte bars follow
the parameter. Reset to `windows-1252` afterward.

The runner saves only successful executions and requires a separate destination for
parameter overrides. Invalid overrides and failed runs preserve both the default and
last successful experiment. Generated validation logs/copies are ignored; the default
notebook keeps its saved outputs.

## Source and interpretation

Inputs are fixed, synthetic examples written in visible SQL: Windows-1252 accents,
a short Shift_JIS sample, a UTF-16LE BOM greeting and a UTF-8 emoji greeting. Expected
text and source labels are independently specified. The real worker performs all
character operations; Python does not substitute its own codecs. Original bytes
remain alongside both decoding paths.

`detect_confidence` is a decode-cleanliness proxy, not a calibrated probability.
The short Japanese fixture gets a wrong guess with a proxy of 1.0. The ambiguous
`C3A9` byte example deliberately has no asserted intended text. The mojibake controls
distinguish known corruption from intentionally literal characters; a proposed repair
can change the intended message. These cases do not estimate population accuracy.

An empty BLOB is valid UTF-8 but has no detection evidence; a missing BLOB remains
NULL. Explicit UTF-8 decoding of the invalid `FF` byte produces U+FFFD. Successful
execution, byte validity and correct interpretation therefore answer different questions.

Export preservation means exact Unicode-text equality after decoding exported bytes
with the same label. Legacy encoders can emit literal HTML numeric references for
unrepresentable characters. No HTML unescaping is applied. Byte counts exclude file
or transport metadata and differ from character counts. Eight SQL relationship checks
plus the selected-codec preflight validate the demonstrated contracts.

**Release-specific limitation:** v0.4.0 emits UTF-8 bytes for UTF-16LE/BE exports.
A real released-runtime probe of `transcode('hé','UTF-16LE')` yields `68C3A9`, while
current-public source yields the expected `6800E900`. Current source also rejects the
decode-only `replacement` export label. This notebook retains the three verified
export choices and separately demonstrates valid UTF-16 BOM decoding. A supported
label is not proof that every export path works in this release. Source checks and
the released artifact must not be described as interchangeable.

## Shared tools and coverage

The [shared standard and runner](https://github.com/Query-farm/vgi-example-notebooks/blob/v0.3.8/docs/standard.md)
are pinned at **v0.3.8**, with a linked catalog title, subtitle and worker repository links. Presentation
and execution helpers are shared rather than copied. Figures retain native SVG plus
content-addressed `charts/` assets for GitHub. Experiment copies keep native SVG.
Retain prior published assets when updating notebooks.

[Coverage evidence](coverage.json) verifies **8/8** logical objects in released v0.4.0
and the reviewed current-public interface. The supported-encodings table/function
aliases count once. Complete runtime function/table/view exports, execution hashes and
explicit narrative/result reviews are retained under `coverage/`. Current-source
catalog discovery is separate from the release used analytically. Both interfaces
have the same eight names; current source fixes the UTF-16/replacement export behavior
outside this notebook's selected export domain. This is interface coverage, not a claim
that every encoding or overload has been tested, or that the two implementations are identical.
