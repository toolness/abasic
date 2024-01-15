import { default as wasm, JsInterpreter, JsInterpreterState, JsInterpreterOutputType, } from "../pkg/abasic_web.js";
import * as ui from "./ui.js";
import { unreachable } from "./util.js";
const VERSION = "0.3.0";
class Interpreter {
    constructor(impl) {
        this.impl = impl;
        /**
         * If we're not fully interactive, then reaching the end of
         * program execution normally will stop the interpreter and
         * not allow further user input. However, pressing CTRL-C
         * to break the program is still allowed, at which point we
         * will be fully interactive.
         *
         * When we're fully interactive, we will present the user
         * with a BASIC prompt at the end of program execution.
         */
        this.isFullyInteractive = true;
        this.handleCurrentState = () => {
            this.showOutput();
            const state = this.impl.get_state();
            switch (state) {
                case JsInterpreterState.Idle:
                    if (!this.isFullyInteractive) {
                        ui.clearPromptAndDisableInput();
                        return;
                    }
                    ui.setPrompt("] ");
                    break;
                case JsInterpreterState.AwaitingInput:
                    ui.setPrompt("? ");
                    break;
                case JsInterpreterState.Errored:
                    const err = this.impl.take_latest_error();
                    if (err === undefined) {
                        throw new Error("Assertion failure, take_latest_error() returned undefined!");
                    }
                    err.split("\n").map((line, i) => {
                        const className = i == 0 ? "error" : "error-context";
                        ui.printSpanWithClass(`${line}\n`, className);
                    });
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
        this.impl.randomize(BigInt(Date.now()));
    }
    loadAndRunSourceCode(sourceCode) {
        this.isFullyInteractive = false;
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
        if (this.isFullyInteractive) {
            ui.print(`Welcome to Atul's BASIC Interpreter v${VERSION}.\n`);
        }
        this.handleCurrentState();
    }
    canProcessUserInput() {
        const state = this.impl.get_state();
        return (state === JsInterpreterState.Idle ||
            state === JsInterpreterState.AwaitingInput);
    }
    canBreak() {
        const state = this.impl.get_state();
        return state !== JsInterpreterState.Idle;
    }
    submitUserInput(input) {
        const state = this.impl.get_state();
        if (state === JsInterpreterState.Idle) {
            this.impl.start_evaluating(input);
            ui.setPrompt("");
        }
        else if (state === JsInterpreterState.AwaitingInput) {
            this.impl.provide_input(input);
            ui.setPrompt("");
        }
        else {
            throw new Error(`submitUserInput called when state is ${JsInterpreterState[state]}!`);
        }
        this.handleCurrentState();
    }
    breakAtCurrentLocation() {
        const state = this.impl.get_state();
        if (state === JsInterpreterState.AwaitingInput ||
            state === JsInterpreterState.Running) {
            this.isFullyInteractive = true;
            ui.commitCurrentPromptToOutput();
            ui.setPrompt("");
            this.impl.break_at_current_location();
            this.handleCurrentState();
        }
    }
    showOutput() {
        const output = this.impl.take_latest_output();
        for (const item of output) {
            switch (item.output_type) {
                case JsInterpreterOutputType.Print:
                    ui.print(item.into_string());
                    break;
                case JsInterpreterOutputType.Trace:
                    ui.printSpanWithClass(`${item.into_string()} `, "info");
                    break;
                case JsInterpreterOutputType.Break:
                case JsInterpreterOutputType.ExtraIgnored:
                case JsInterpreterOutputType.Reenter:
                case JsInterpreterOutputType.Warning:
                    ui.printSpanWithClass(`${item.into_string()}\n`, "warning");
                    break;
                default:
                    unreachable(item.output_type);
            }
        }
    }
}
/**
 * Programs to run can be specified via a relative path or just the
 * stem of the program to run. If it's the latter, we'll expand it
 * to a relative path ourselves.
 *
 * Note: we don't actually check to make sure the path is a relative
 * URL or anything; users could pass in an absolute URL and who
 * knows what will happen. This is not that big a deal since this is
 * just a fun experiment.
 */
function normalizeProgramPath(path) {
    if (!path?.trim()) {
        return null;
    }
    if (/^[A-Za-z0-9]+$/.test(path)) {
        return `programs/${path}.bas`;
    }
    return path;
}
wasm().then(async (module) => {
    const interpreter = new Interpreter(JsInterpreter.new());
    const searchParams = new URLSearchParams(window.location.search);
    const programPath = normalizeProgramPath(searchParams.get("p"));
    if (programPath) {
        const sourceCodeRequest = await fetch(programPath);
        if (!sourceCodeRequest.ok) {
            ui.printSpanWithClass(`\nFailed to load ${programPath} (HTTP ${sourceCodeRequest.status}).\n`, "error");
            return;
        }
        const sourceCode = await sourceCodeRequest.text();
        ui.clearScreen();
        interpreter.loadAndRunSourceCode(sourceCode);
    }
    else {
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
            interpreter.breakAtCurrentLocation();
        }
    });
    ui.onSubmitInput(() => {
        const input = ui.getInput();
        // If the user is on a phone or tablet, they're not going to be able to press CTRL-C,
        // so we'll just treat this special emoji as the same thing.
        if (interpreter.canBreak() && input === "💥") {
            ui.clearInput();
            interpreter.breakAtCurrentLocation();
            return;
        }
        if (!interpreter.canProcessUserInput()) {
            return;
        }
        ui.commitCurrentPromptToOutput(input);
        interpreter.submitUserInput(input);
        ui.clearInput();
    });
});
