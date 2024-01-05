ABASIC is a simple Rust-based first-generation BASIC interpreter.

This interpreter was made primarily for learning and personal edification.
It is [missing features](#limitations) from mainstream implementations
of the language, so if you want a full-featured BASIC, you should probably
look elsewhere.

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

* `DEF` (user-definable functions)
* `LET`
* `IF ... THEN ... {ELSE}`
* `FOR ... TO ... {STEP} ... NEXT`
* `GOTO`
* `GOSUB`
* `REM`
* `PRINT` / `?`
* `INPUT`
* `READ`, `RESTORE`, and `DATA`
* `DIM` (arrays)
* Arithmetic expressions (`+`, `-`, `*`, `/`, and `^`)
* Logical operators (`AND`, `OR`, `NOT`)
* Floating point and string values
* Line crunching (e.g., `10PRINT123` is semantically identical to
  `10 PRINT 123`)
* `:` (used to execute multiple statements in one line)

The interpreter also supports a number of debugging features inspired by
Applesoft BASIC:

* `STOP` is similar to JavaScript's `debugger` instruction, and will pause
  a program's execution. At this point, the program state can be inspected
  and changed. `CONT` can be used to resume execution.

* Pressing CTRL-C during execution will pause a program's execution at
  whatever line it's currently on. At this point the program can be
  inspected and resumed with `CONT`.

* `TRACE` can be used to show the line number of each statement as it's
  being executed.  To disable the feature, use `NOTRACE`.

## Limitations

There's a lot of things that haven't been implemented, some of which include:

* `WHILE ... WEND`, `REPEAT ... UNTIL`, `DO ... LOOP`
* `ON ... GOTO/GOSUB`
* Scientific notation (e.g. `1.3E-4`)
* Integer types via the `%` suffix (e.g. `C% = 1`)
* `MAT` (matrices)

## Other notes

* BASIC was never standardized (attempts were made, but they never succeeded).
  Because of this, users inputting BASIC programs listed in books and
  magazines had to have some knowledge of BASIC: publications usually included
  tips on modifications needed to make their programs work on individual
  platforms, but always told users that they needed to read their platform's
  BASIC manual and deal with idiosyncrasies as they encountered them.

  This gave me a surprising amount of creative freedom in implementing ABASIC.
  While I usually hewed to the behavior of Applesoft BASIC, I also examined
  the behavior of Dartmouth BASIC and sometimes chose behavior based on
  whatever seemed most user-friendly or historically accurate.

* Reading about the history of BASIC made me realize that line numbers were
  actually a vital affordance, as the language was invented during a time
  when computers didn't have screens--they just had keyboards and printers.
  Essentially, communicating with a computer back then was a bit like
  talking via SMS text messaging today.

  This meant that document editors couldn't really exist. In lieu of that,
  assigning numbers to lines and allowing them to be redefined interactively
  actually allowed for a surprisingly fast feedback loop.

* ABASIC provides no limitations on variable and function names. This is
  unlike most first-generation BASIC interpreters, which only permitted short
  variable names, likely to preserve memory (Applesoft actually permitted
  long names but ignored everything after the first two characters).

  At first I assumed this was an unquestionably good thing, but then I
  realized that since original BASIC ignores whitespace, long variable
  names can lead to surprising syntax errors, because the likelihood of
  a reserved word like `IF` or `NOT` appearing somewhere in a variable
  name increases as its length grows. This is probably part of why
  later BASICs abandoned line crunching.

* ABASIC's interpreter uses a recursive descent parser, evaluating the code
  as it's being parsed. In other words, there is no abstract syntax tree
  (AST).

  As far as I can tell, this appears to be how Applesoft BASIC works too; for
  example, the following line runs fine:

  ```basic
  GOTO 10BLARGBLARGO#@$OWEJR
  ```

  If Applesoft had parsed the code into an AST before evaluating it, it would
  have raised a syntax error. Instead, it seems to be seeing the `GOTO 10`
  and immediately moving to line 10, ignoring the rest of the line.

  ABASIC works in a similar way. I'm guessing that in Applesoft's case it was
  done to preserve memory; in ABASIC's case, it was mostly done for
  expediency, though I also liked the idea of preserving the behavior of
  first-generation BASIC interpreters.

* ABASIC has the optional ability to warn users about suspect code,
  such as when a variable that has never been assigned to is used in an
  expression (in such cases the variable defaults to zero or an empty string,
  as per BASIC's traditional behavior).

  This feature can be enabled via the `-w` flag on the command-line.

## Other resources

* [Dartmouth BASIC Fourth Edition language guide (1968)](https://archive.org/details/bitsavers_dartmouthB_3679804)

* [Applesoft II BASIC Reference Manual (1978)](https://archive.org/details/applesoft-ii-ref/page/n33/mode/2up)

* [Wikipedia page on BASIC](https://en.wikipedia.org/wiki/BASIC)

## License

Everything in this repository not expressly attributed to other sources is licensed under [CC0 1.0 Universal](./LICENSE.md) (public domain).
