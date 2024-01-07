import {
  default as wasm,
  init_and_set_rnd_seed,
  JsInterpreter,
  JsInterpreterState,
  JsInterpreterOutputType,
} from "../pkg/abasic_web.js";
import * as ui from "./ui.js";
import { unreachable } from "./util.js";

class Interpreter {
  constructor(private readonly impl: JsInterpreter) {}

  loadAndRunSourceCode(sourceCode: string) {
    let lines = sourceCode.split("\n");
    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }
      if (!/^[0-9]/.test(line)) {
        console.warn("Skipping line, as it's not numbered:", line);
        continue;
      }
      this.impl.start_evaluating(line);
    }
    this.impl.start_evaluating("RUN");
  }

  start() {
    this.handleCurrentState();
  }

  canProcessUserInput(): boolean {
    const state = this.impl.get_state();
    return (
      state === JsInterpreterState.Idle ||
      state === JsInterpreterState.AwaitingInput
    );
  }

  submitUserInput(input: string) {
    const state = this.impl.get_state();
    if (state === JsInterpreterState.Idle) {
      this.impl.start_evaluating(input);
      ui.setPrompt("");
    } else if (state === JsInterpreterState.AwaitingInput) {
      this.impl.provide_input(input);
      ui.setPrompt("");
    } else {
      throw new Error(
        `submitUserInput called when state is ${JsInterpreterState[state]}!`
      );
    }
    this.handleCurrentState();
  }

  break() {
    const state = this.impl.get_state();
    if (
      state === JsInterpreterState.AwaitingInput ||
      state === JsInterpreterState.Running
    ) {
      ui.commitCurrentPromptToOutput();
      ui.setPrompt("");
      this.impl.break_at_current_location();
      this.handleCurrentState();
    }
  }

  private showOutput() {
    const output = this.impl.take_latest_output();
    for (const item of output) {
      if (item.output_type === JsInterpreterOutputType.Print) {
        ui.print(item.into_string());
      } else {
        // TODO: Print this in a different color?
        ui.print(`${item.into_string()}\n`);
      }
    }
  }

  private handleCurrentState = () => {
    this.showOutput();
    const state = this.impl.get_state();
    switch (state) {
      case JsInterpreterState.Idle:
        ui.setPrompt("] ");
        break;
      case JsInterpreterState.AwaitingInput:
        ui.setPrompt("? ");
        break;
      case JsInterpreterState.Errored:
        const err = this.impl.take_latest_error();
        if (err === undefined) {
          throw new Error(
            "Assertion failure, take_latest_error() returned undefined!"
          );
        }
        ui.print(err);
        this.handleCurrentState();
        break;
      case JsInterpreterState.Running:
        this.impl.continue_evaluating();
        window.setTimeout(this.handleCurrentState, 5);
        break;
      default:
        unreachable(state);
    }
  };
}

wasm().then(async (module) => {
  init_and_set_rnd_seed(BigInt(Date.now()));

  const interpreter = new Interpreter(JsInterpreter.new());

  const searchParams = new URLSearchParams(window.location.search);
  const programPath = searchParams.get("p");
  if (programPath) {
    const sourceCodeRequest = await fetch(programPath);
    if (!sourceCodeRequest.ok) {
      ui.print(
        `Failed to load ${programPath} (HTTP ${sourceCodeRequest.status}).\n`
      );
      return;
    }
    const sourceCode = await sourceCodeRequest.text();
    ui.clearScreen();
    interpreter.loadAndRunSourceCode(sourceCode);
  } else {
    ui.clearScreen();
  }

  interpreter.start();

  ui.onInputKeyDown((event, inputEl) => {
    // We want to process CTRL-C, but we need to be careful not to break when
    // users on some platforms (e.g. Windows) are just trying to copy text to
    // the clipboard.
    if (inputEl.selectionStart !== inputEl.selectionEnd) {
      return;
    }
    if (event.ctrlKey && event.key.toUpperCase() === "C") {
      event.preventDefault();
      interpreter.break();
    }
  });

  ui.onSubmitInput(() => {
    const input = ui.getInput();

    if (!interpreter.canProcessUserInput()) {
      // If the user is on a phone or tablet, they're not going to be able to press CTRL-C,
      // so we'll just treat this special emoji as the same thing.
      if (input === "💥") {
        ui.clearInput();
        interpreter.break();
      }
      return;
    }

    ui.commitCurrentPromptToOutput(input);
    interpreter.submitUserInput(input);
    ui.clearInput();
  });
});
