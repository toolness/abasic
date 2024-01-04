This is a simple Rust-based BASIC interpreter with the following goals:

- I programmed BASIC as a kid but have largely forgotten it. I also
  didn't fully understand the syntax and semantics back then.
  I've encountered the language recently when attempting to learn
  [6502 assembly][] for the Apple II, so I thought writing my own
  interpreter would help me learn the language better.

- I hadn't written anything in Rust in a while and needed an excuse.

- I've been reading about the history of vintage software in books
  like [50 Years of Text Games][], [The Apple II Age][], and
  [101 BASIC Computer Games][] and thought it'd be fun to make an
  interpreter capable of running some of those programs.

[6502 assembly]: https://github.com/toolness/apple-6502-fun/
[50 Years of Text Games]: https://aaronareed.net/50-years-of-text-games/
[The Apple II Age]: https://press.uchicago.edu/ucp/books/book/chicago/A/bo195231688.html
[101 BASIC Computer Games]: https://en.wikipedia.org/wiki/BASIC_Computer_Games

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

## License

Everything in this repository not expressly attributed to other sources is licensed under [CC0 1.0 Universal](./LICENSE.md) (public domain).
