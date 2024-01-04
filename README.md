This is a simple Rust-based BASIC interpreter.

This interpreter was made primarily for personal edification. It is
[missing features](#limitations) from most mainstream implementations of the language,
so if you want a full-featured BASIC, you should probably look elsewhere.

For more details, see [Rationale](#Rationale).

## Quick start

Install the executable with:

<!-- Note that the `cd` here is annoying, we can remove it if this ever gets fixed: https://github.com/rust-lang/cargo/issues/4101 -->

```
cd abasic_core
cargo install --path=.
```

You can run the BASIC interpreter interactively with:

```
abasic
```

Or you can run a program:

```
abasic programs/chemist.bas
```

Use `abasic --help` for more details.

## Rationale

I created this interpreter with the following goals:

- I programmed BASIC as a child but have largely forgotten it. I also
  didn't fully understand the syntax and semantics back then.
  I've encountered the language recently when attempting to learn
  [6502 assembly][] for the Apple II and have been confused by some
  of its more idiosyncratic aspects, so I thought writing my own
  interpreter would help me learn the language better.

- I've been reading about historic software in books
  like [50 Years of Text Games][], [The Apple II Age][], and
  [101 BASIC Computer Games][] and thought it'd be fun to make an
  interpreter capable of running some of those programs.

- I hadn't written anything in Rust in a while and wanted an excuse.

[6502 assembly]: https://github.com/toolness/apple-6502-fun/
[50 Years of Text Games]: https://aaronareed.net/50-years-of-text-games/
[The Apple II Age]: https://press.uchicago.edu/ucp/books/book/chicago/A/bo195231688.html
[101 BASIC Computer Games]: https://en.wikipedia.org/wiki/BASIC_Computer_Games

## What's implemented

Note that this list isn't exhaustive.

* `IF ... THEN ... {ELSE}`
* `FOR ... TO ... {STEP} ... NEXT`
* `GOTO`
* `GOSUB`
* `REM`
* `PRINT`
* `INPUT`
* `READ`, `RESTORE`, and `DATA`
* `DIM` (arrays)
* Arithmetic expressions (`+`, `-`, `*`, `/`, and `^`)
* Logical operators (`AND`, `OR`, `NOT`)
* Floating point and string values
* Line crunching (e.g., `10PRINT123` is semantically identical to `10 PRINT 123`)

## Limitations

There's a lot of things that haven't been implemented, some of which include:

* `DEF` (user-definable functions)
* `WHILE ... WEND`, `REPEAT ... UNTIL`, `DO ... LOOP`
* `ON ... GOTO/GOSUB`
* Scientific notation (e.g. `1.3E-4`)
* Integer types via the `%` suffix (e.g. `C% = 1`)
* `MAT` (matrices)

## License

Everything in this repository not expressly attributed to other sources is licensed under [CC0 1.0 Universal](./LICENSE.md) (public domain).
