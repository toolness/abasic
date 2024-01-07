import { default as wasm, JsInterpreter, JsInterpreterState, JsInterpreterOutputType } from "./pkg/abasic_web.js";

const a11yOutputEl = el_with_id('a11y-output');
const outputEl = el_with_id('output');
const promptEl = el_with_id('prompt');
const inputEl = el_with_id('input');
const formEl = el_with_id('form');

if (!(inputEl instanceof HTMLInputElement))
    throw new Error("Expected inputEl to be an <input>");

if (!(formEl instanceof HTMLFormElement))
    throw new Error("Expected formEl to be a <form>");

function print(msg: string) {
    const textNode = document.createTextNode(msg);
    outputEl.appendChild(textNode);
    a11yOutputEl.appendChild(textNode.cloneNode());
    scroll_output();
}

function clearScreen() {
    outputEl.textContent = "";
    scroll_output();
}

// See our CSS for .ugh-ios for details on why we're doing this.
const IS_IOS = /iPad|iPhone|iPod/.test(navigator.userAgent) && !('MSStream' in window);
if (IS_IOS) {
    document.documentElement.classList.add('ugh-ios');
}

function el_with_id(id: string): HTMLElement {
    const el = document.getElementById(id);
    if (el === null)
        throw new Error(`Element with id "${id}" not found!`);
    return el;
}

function scroll_output() {
    // Different browsers use different elements for scrolling. :(
    [document.documentElement, document.body].forEach(el => {
        el.scrollTop = el.scrollHeight;
    });
}

const setPrompt = (prompt: string) => {
    promptEl.textContent = prompt;
    a11yOutputEl.appendChild(document.createTextNode(prompt));
};

class Interpreter {
    constructor(private readonly impl: JsInterpreter) {
    }

    start() {
        this.handleCurrentState();
    }

    canProcessUserInput(): boolean {
        const state = this.impl.get_state();
        return state === JsInterpreterState.Idle || state === JsInterpreterState.AwaitingInput
    }

    submitUserInput(input: string) {
        const state = this.impl.get_state();
        if (state === JsInterpreterState.Idle) {
            this.impl.start_evaluating(input);
            setPrompt("");
        } else if (state === JsInterpreterState.AwaitingInput) {
            this.impl.provide_input(input);
            setPrompt("");
        } else {
            throw new Error(`submitUserInput called when state is ${JsInterpreterState[state]}!`);
        }
        this.handleCurrentState();
    }

    break() {
        const state = this.impl.get_state();
        if (state === JsInterpreterState.AwaitingInput || state === JsInterpreterState.Running) {
            setPrompt("");
            this.impl.break_at_current_location();
            this.handleCurrentState();
        }
    }

    private showOutput() {
        const output = this.impl.take_latest_output();
        for (const item of output) {
            if (item.output_type  === JsInterpreterOutputType.Print) {
                print(item.into_string());
            } else {
                // TODO: Print this in a different color?
                print(`${item.into_string()}\n`);
            }
        }
    }

    private handleCurrentState = () => {
        this.showOutput();
        const state = this.impl.get_state();
        switch (state) {
            case JsInterpreterState.Idle:
                setPrompt("] ");
                break;
            case JsInterpreterState.AwaitingInput:
                setPrompt("? ");
                break;
            case JsInterpreterState.Errored:
                const err = this.impl.take_latest_error();
                if (err === undefined) {
                    throw new Error("Assertion failure, take_latest_error() returned undefined!")
                }
                print(err);
                this.handleCurrentState();
                break;
            case JsInterpreterState.Running:
                this.impl.continue_evaluating();
                window.setTimeout(this.handleCurrentState, 5);
                break;
            default:
                unreachable(state);
        }
    }
}

function unreachable(arg: never) {
    throw new Error(`Assertion failure, unreachable(${arg}) was called!`);
}

wasm().then((module) => {
    clearScreen();

    const interpreter = new Interpreter(JsInterpreter.new());

    interpreter.start();

    window.addEventListener('keydown', event => {
        if (event.ctrlKey && event.key.toUpperCase() === 'C') {
            interpreter.break();
        }
    });

    formEl.addEventListener('submit', e => {
        e.preventDefault();

        if (!interpreter.canProcessUserInput()) {
            // If the user is on a phone or tablet, they're not going to be able to press CTRL-C,
            // so we'll just treat this special emoji as the same thing.
            if (inputEl.value === "💥") {
                inputEl.value = "";
                interpreter.break();
            }
            return;
        }

        const el = document.createElement('div');

        el.setAttribute('class', 'prompt-response');
        el.textContent = `${promptEl.textContent}${inputEl.value}`;
        outputEl.appendChild(el);
        scroll_output();

        interpreter.submitUserInput(inputEl.value);
        inputEl.value = "";
    });
});
