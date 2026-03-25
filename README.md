# spooky_go 🎃👻

Rust and Python library for the game of Go.

# Features

- Drive external engines with [Go Text Protocol](https://en.wikipedia.org/wiki/Go_Text_Protocol).
- Variable board sizes from 4x4 to 32x32.
- Relatively fast.
- Out-of-the-box support for DL/ML (action encoding and decoding methods).
- Consistent interface with [spooky-chess](https://github.com/snowdrop4/spooky-chess) and [spooky-connect4](https://github.com/snowdrop4/spooky-connect4).

# Install

## Rust

```fish
cargo add spooky_go
```

## Python

```fish
uv add spooky-go
```

Includes type hints.

# Develop

### Tests

- `fish run_tests.fish`
    - `fish run_python_tests.fish`
    - `fish run_rust_tests.fish`

### Lints

- `fish run_lints.fish`

### Performance

- `fish run_benchmark.fish`

# See Also

* [spooky-chess](https://github.com/snowdrop4/spooky-chess)
* [spooky-connect4](https://github.com/snowdrop4/spooky-connect4)
* spooky-go
