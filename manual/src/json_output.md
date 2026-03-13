# JSON Output

Use `--display=json` to print a machine-readable diff instead of the
terminal UI.

```bash
$ difft --display=json sample_files/simple_1.js sample_files/simple_2.js
```

For a single file diff, difftastic prints one JSON object. For a
directory diff, it prints an array of file objects.

The JSON output is intended for tools and scripts, and its structure may
change in future releases.

## Top-Level Fields

Each file object always has these fields:

| Field | Type | Description |
| --- | --- | --- |
| `language` | string | The detected language, or `Binary`. |
| `path` | string | The display path for the diffed file. |
| `status` | string | One of `changed`, `unchanged`, `created`, or `deleted`. |

Changed text files also include:

| Field | Type | Description |
| --- | --- | --- |
| `aligned_lines` | array | Pairs of aligned line numbers. Each pair is `[lhs_line, rhs_line]`, with `null` when a line only exists on one side. |
| `chunks` | array | The changed hunks. Each hunk is an array of changed lines. |

If a file is unchanged, created, deleted, or treated as binary,
`aligned_lines` and `chunks` are omitted.

## Line Objects

Each entry in `chunks` is an object with optional `lhs` and `rhs`
fields. The fields are omitted when a line only exists on the other
side.

```json
{
  "lhs": {
    "line_number": 0,
    "changes": [
      {
        "start": 0,
        "end": 5,
        "content": "const",
        "highlight": "keyword"
      }
    ]
  },
  "rhs": {
    "line_number": 0,
    "changes": [
      {
        "start": 0,
        "end": 6,
        "content": "import",
        "highlight": "keyword"
      }
    ]
  }
}
```

- `line_number` is zero-based.
- `changes` contains the novel regions on that line.
- `start` and `end` are zero-based offsets within the line, and `end` is
  exclusive.
- `content` is the exact substring for that changed region.
- `highlight` describes the syntax category. Current values are
  `delimiter`, `normal`, `string`, `type`, `comment`, `keyword`, and
  `tree_sitter_error`.

## Example

This is the output for:

```bash
$ difft --display=json sample_files/simple_1.js sample_files/simple_2.js
```

```json
{
  "aligned_lines": [
    [0, 0],
    [1, 1],
    [2, 2]
  ],
  "chunks": [
    [
      {
        "lhs": {
          "line_number": 0,
          "changes": [
            {
              "start": 0,
              "end": 5,
              "content": "const",
              "highlight": "keyword"
            },
            {
              "start": 12,
              "end": 13,
              "content": "=",
              "highlight": "keyword"
            },
            {
              "start": 14,
              "end": 21,
              "content": "require",
              "highlight": "normal"
            },
            {
              "start": 21,
              "end": 22,
              "content": "(",
              "highlight": "delimiter"
            },
            {
              "start": 29,
              "end": 30,
              "content": ")",
              "highlight": "delimiter"
            }
          ]
        },
        "rhs": {
          "line_number": 0,
          "changes": [
            {
              "start": 0,
              "end": 6,
              "content": "import",
              "highlight": "keyword"
            },
            {
              "start": 12,
              "end": 13,
              "content": ",",
              "highlight": "normal"
            },
            {
              "start": 14,
              "end": 15,
              "content": "{",
              "highlight": "delimiter"
            },
            {
              "start": 15,
              "end": 23,
              "content": "useState",
              "highlight": "normal"
            },
            {
              "start": 23,
              "end": 24,
              "content": "}",
              "highlight": "delimiter"
            },
            {
              "start": 25,
              "end": 29,
              "content": "from",
              "highlight": "keyword"
            }
          ]
        }
      }
    ]
  ],
  "language": "JavaScript",
  "path": "sample_files/simple_2.js",
  "status": "changed"
}
```
