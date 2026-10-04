"""docs/examples.md の Python のコード例を実行し、`# =>` の後に書いた出力と一致することを確かめる。

各節は `<!-- example: KEY -->` で始まり、その節の ```python のブロックを実行する。
ブロックの `print(...)` の行は、行末の `  # => ` の後に、その行が出力する文字列を書く。
"""
import contextlib
import io
import pathlib
import re

import pytest

EXAMPLES = pathlib.Path(__file__).resolve().parent.parent.parent / "docs" / "examples.md"
MARKER = re.compile(r"^<!-- example: (\w+) -->$", re.MULTILINE)
PYTHON_BLOCK = re.compile(r"^```python\n(.*?)^```$", re.MULTILINE | re.DOTALL)
EXPECTED = re.compile(r"^print\(.*\)  # => (.*)$")


def sections():
    text = EXAMPLES.read_text(encoding="utf-8")
    markers = list(MARKER.finditer(text))
    result = {}
    for k, marker in enumerate(markers):
        end = markers[k + 1].start() if k + 1 < len(markers) else len(text)
        blocks = PYTHON_BLOCK.findall(text, marker.end(), end)
        assert len(blocks) == 1, f"{marker.group(1)}: python のブロックが {len(blocks)} 個"
        assert marker.group(1) not in result, f"{marker.group(1)}: KEY が重複している"
        result[marker.group(1)] = blocks[0]
    return result


SECTIONS = sections()


def test_has_sections():
    assert len(SECTIONS) > 0


@pytest.mark.parametrize("key", list(SECTIONS))
def test_example(key):
    code = SECTIONS[key]
    expected = []
    for line in code.splitlines():
        if line.startswith("print("):
            match = EXPECTED.match(line)
            assert match, f"print の行に `  # => 出力` がない: {line}"
            expected.append(match.group(1))
    assert expected, "print の行がない"

    output = io.StringIO()
    with contextlib.redirect_stdout(output):
        exec(compile(code, f"examples.md:{key}", "exec"), {})
    assert output.getvalue().splitlines() == expected
